use async_trait::async_trait;
use diraigent_orchestra::mcp::*;
use diraigent_types::{mcp::*, task_profile::TaskProfile};
use serde_json::{Value, json};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
    time::Duration,
};
use uuid::Uuid;
use wiremock::{
    Mock, MockServer, Request, Respond, ResponseTemplate,
    matchers::{body_partial_json, method},
};

struct Policy(Mutex<Approval>);
#[async_trait]
impl Registry for Policy {
    async fn current(&self, project: Uuid, server: Uuid) -> Result<Approval, BrokerError> {
        let a = self.0.lock().unwrap().clone();
        if project != a.project_id || server != a.server_id {
            return Err(BrokerError::Denied);
        }
        Ok(a)
    }
}
#[derive(Default)]
struct Audit(Mutex<Vec<McpAuditEvent>>);
impl AuditSink for Audit {
    fn record(&self, event: McpAuditEvent) {
        self.0.lock().unwrap().push(event);
    }
}
fn approval(transport: McpTransport) -> Approval {
    Approval {
        project_id: Uuid::new_v4(),
        server_id: Uuid::new_v4(),
        revision: 1,
        approved_revision: Some(1),
        enabled: true,
        allow_private_network: false,
        configuration: McpConfiguration {
            name: "fake".into(),
            transport,
            tools: vec![
                McpToolPermission {
                    name: "read".into(),
                    access: McpToolAccess::Read,
                },
                McpToolPermission {
                    name: "write".into(),
                    access: McpToolAccess::Write,
                },
            ],
            credential_bindings: BTreeMap::new(),
        },
    }
}
fn limits() -> Limits {
    Limits {
        connect_timeout: Duration::from_secs(3),
        call_timeout: Duration::from_millis(300),
        lifetime: Duration::from_secs(10),
        max_bytes: 4096,
        ..Limits::default()
    }
}
struct Fixture {
    _dir: tempfile::TempDir,
    pid: std::path::PathBuf,
    log: std::path::PathBuf,
    approval: Approval,
}
fn fixture(mode: &str) -> Fixture {
    let python = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("python3"))
        .find(|p| p.is_file())
        .expect("MCP fake-server tests require preinstalled python3")
        .canonicalize()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let pid = dir.path().join("pid");
    let log = dir.path().join("log");
    let script =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_server.py");
    let a = approval(McpTransport::Stdio {
        executable: python.to_str().unwrap().into(),
        arguments: vec![
            script.to_str().unwrap().into(),
            mode.into(),
            pid.to_str().unwrap().into(),
            log.to_str().unwrap().into(),
        ],
    });
    Fixture {
        _dir: dir,
        pid,
        log,
        approval: a,
    }
}
async fn stopped(f: &Fixture) {
    let pid: i32 = std::fs::read_to_string(&f.pid).unwrap().parse().unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        while unsafe { libc::kill(pid, 0) } == 0 {
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("subprocess must be killed and reaped");
}
async fn connect(
    a: Approval,
    profile: TaskProfile,
    l: Limits,
) -> (Broker, TaskAccess, Arc<Policy>, Arc<Audit>) {
    let policy = Arc::new(Policy(Mutex::new(a.clone())));
    let audit = Arc::new(Audit::default());
    let (broker, access, check) = Broker::connect(
        a,
        Uuid::new_v4(),
        profile,
        Credentials(BTreeMap::from([(
            "token".into(),
            "fake-upstream-secret".into(),
        )])),
        policy.clone(),
        audit.clone(),
        l,
    )
    .await
    .unwrap();
    assert_eq!(check.status, McpCheckStatus::Connected);
    assert!(!check.approved_tools.contains(&"unapproved".to_owned()));
    (broker, access, policy, audit)
}

#[tokio::test]
async fn mcp_stdio_exact_policy_credentials_and_isolation() {
    let mut f = fixture("normal");
    f.approval
        .configuration
        .credential_bindings
        .insert("FIXTURE_TOKEN".into(), "token".into());
    let (broker, access, policy, audit) =
        connect(f.approval.clone(), TaskProfile::Review, limits()).await;
    let list = broker.tools_list(&access).await.unwrap();
    assert_eq!(list["tools"].as_array().unwrap().len(), 1);
    for name in [
        "write",
        "unapproved",
        "READ",
        "read*",
        "resources/list",
        "prompts/get",
    ] {
        assert_eq!(
            broker.tools_call(&access, name, json!({})).await,
            Err(BrokerError::Denied)
        );
    }
    let result = broker.tools_call(&access, "read", json!({})).await.unwrap();
    assert_eq!(result["credential_bound"], true);
    assert_eq!(result["ambient_absent"], true);
    let other = fixture("normal");
    let (other_broker, other_access, _, _) =
        connect(other.approval.clone(), TaskProfile::Execute, limits()).await;
    assert_eq!(
        broker.tools_list(&other_access).await,
        Err(BrokerError::Denied)
    );
    assert_eq!(
        other_broker.tools_call(&access, "read", json!({})).await,
        Err(BrokerError::Denied)
    );
    policy.0.lock().unwrap().revision = 2;
    policy.0.lock().unwrap().approved_revision = Some(2);
    assert_eq!(
        broker.tools_call(&access, "read", json!({})).await,
        Err(BrokerError::Stale)
    );
    let events = serde_json::to_string(&*audit.0.lock().unwrap()).unwrap();
    assert!(!events.contains("fake-upstream-secret"));
    assert!(!events.contains("unapproved"));
    let log = std::fs::read_to_string(&f.log).unwrap();
    assert_eq!(log.lines().filter(|l| *l == "tools/call").count(), 1);
    assert!(!log.contains("resources/") && !log.contains("prompts/"));
    drop(broker);
    drop(other_broker);
    stopped(&f).await;
    stopped(&other).await;
}

#[tokio::test]
async fn mcp_changed_and_new_catalogs_are_not_execution_grants() {
    for mode in ["changed", "new"] {
        let f = fixture(mode);
        let (broker, access, _, _) =
            connect(f.approval.clone(), TaskProfile::Execute, limits()).await;
        assert_eq!(broker.tools_list(&access).await, Err(BrokerError::Stale));
        assert!(
            !std::fs::read_to_string(&f.log)
                .unwrap()
                .contains("tools/call")
        );
        stopped(&f).await;
    }
    let f = fixture("changed");
    let (broker, access, _, _) = connect(f.approval.clone(), TaskProfile::Execute, limits()).await;
    assert_eq!(
        broker.tools_call(&access, "read", json!({})).await,
        Err(BrokerError::Stale)
    );
    stopped(&f).await;
}

#[tokio::test]
async fn mcp_timeout_disconnect_oversized_and_request_cleanup() {
    for (mode, error) in [
        ("hang", BrokerError::Timeout),
        ("disconnect", BrokerError::Disconnected),
        ("oversized", BrokerError::Oversized),
        ("request", BrokerError::Protocol),
    ] {
        let f = fixture(mode);
        let (broker, access, _, _) =
            connect(f.approval.clone(), TaskProfile::Execute, limits()).await;
        assert_eq!(
            broker.tools_call(&access, "read", json!({})).await,
            Err(error)
        );
        drop(broker);
        stopped(&f).await;
    }
}

#[tokio::test]
async fn mcp_cancellation_and_lifetime_cleanup() {
    let f = fixture("hang");
    let (broker, access, _, _) = connect(f.approval.clone(), TaskProfile::Execute, limits()).await;
    assert!(
        tokio::time::timeout(
            Duration::from_millis(100),
            broker.tools_call(&access, "read", json!({}))
        )
        .await
        .is_err()
    );
    stopped(&f).await;
    assert_eq!(
        broker.tools_list(&access).await,
        Err(BrokerError::Disconnected)
    );
    let f = fixture("normal");
    let mut l = limits();
    l.lifetime = Duration::from_millis(100);
    let (_broker, _, _, _) = connect(f.approval.clone(), TaskProfile::Execute, l).await;
    stopped(&f).await;
}

#[tokio::test]
async fn mcp_revocation_and_configuration_tampering_denied() {
    for mode in ["disabled", "config", "project", "permission"] {
        let f = fixture("normal");
        let (broker, access, policy, _) =
            connect(f.approval.clone(), TaskProfile::Execute, limits()).await;
        {
            let mut a = policy.0.lock().unwrap();
            match mode {
                "disabled" => a.enabled = false,
                "config" => a.configuration.name = "tampered".into(),
                "project" => a.project_id = Uuid::new_v4(),
                _ => a.configuration.tools[0].access = McpToolAccess::Write,
            }
        }
        assert!(matches!(
            broker.tools_call(&access, "read", json!({})).await,
            Err(BrokerError::Denied | BrokerError::Stale)
        ));
        assert!(
            !std::fs::read_to_string(&f.log)
                .unwrap()
                .contains("tools/call")
        );
        drop(broker);
        stopped(&f).await;
    }
}

#[derive(Clone)]
struct HttpFixture {
    sse: bool,
}
impl Respond for HttpFixture {
    fn respond(&self, request: &Request) -> ResponseTemplate {
        let body: Value = serde_json::from_slice(&request.body).unwrap();
        assert_eq!(
            request.headers.get("authorization").unwrap(),
            "Bearer fake-upstream-secret"
        );
        assert_eq!(
            request.headers.get("mcp-protocol-version").unwrap(),
            "2025-06-18"
        );
        let method = body["method"].as_str().unwrap();
        if method != "initialize" {
            assert_eq!(
                request.headers.get("mcp-session-id").unwrap(),
                "fake-session"
            );
        }
        let result = match method {
            "initialize" => {
                json!({"protocolVersion":"2025-06-18", "capabilities":{"tools":{}}, "serverInfo":{"name":"fake","version":"1"}})
            }
            "notifications/initialized" => return ResponseTemplate::new(202),
            "tools/list" => json!({"tools":[{"name":"read","inputSchema":{"type":"object"}},
                {"name":"unapproved","inputSchema":{"type":"object"}}]}),
            "tools/call" => {
                assert_eq!(body["params"]["name"], "read");
                json!({"content":[{"type":"text","text":"ok"}]})
            }
            _ => panic!("implicit capability access"),
        };
        let response = json!({"jsonrpc":"2.0", "id":body["id"], "result":result});
        let template = ResponseTemplate::new(200).insert_header("mcp-session-id", "fake-session");
        if self.sse {
            template.set_body_raw(
                format!("event: message\r\ndata: {response}\r\n\r\n"),
                "text/event-stream",
            )
        } else {
            template.set_body_json(response)
        }
    }
}

#[tokio::test]
async fn mcp_http_json_and_streamable_sse_with_session() {
    for sse in [false, true] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(HttpFixture { sse })
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let mut a = approval(McpTransport::StreamableHttp {
            endpoint: server.uri(),
        });
        a.allow_private_network = true; // explicit human-approved local endpoint
        a.configuration
            .credential_bindings
            .insert("authorization".into(), "token".into());
        let policy = Arc::new(Policy(Mutex::new(a.clone())));
        let (broker, access, check) = Broker::connect(
            a,
            Uuid::new_v4(),
            TaskProfile::Research,
            Credentials(BTreeMap::from([(
                "token".into(),
                "Bearer fake-upstream-secret".into(),
            )])),
            policy,
            Arc::new(Audit::default()),
            limits(),
        )
        .await
        .unwrap();
        assert_eq!(check.approved_tools, vec!["read"]);
        assert_eq!(
            broker.tools_call(&access, "unapproved", json!({})).await,
            Err(BrokerError::Denied)
        );
        assert!(broker.tools_call(&access, "read", json!({})).await.is_ok());
        drop(broker);
    }
}

#[tokio::test]
async fn mcp_http_endpoint_policy_redirect_and_wildcard_denied() {
    let server = MockServer::start().await;
    let target = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(ResponseTemplate::new(307).insert_header("location", target.uri()))
        .mount(&server)
        .await;
    for (endpoint, private, expected) in [
        (server.uri(), false, BrokerError::Denied),
        (server.uri(), true, BrokerError::Upstream),
        ("https://127.0.0.1/mcp".into(), false, BrokerError::Denied),
        (
            "https://[::ffff:127.0.0.1]/mcp".into(),
            false,
            BrokerError::Denied,
        ),
        (
            "https://user:pass@example.com/mcp".into(),
            true,
            BrokerError::Denied,
        ),
    ] {
        let mut a = approval(McpTransport::StreamableHttp { endpoint });
        a.allow_private_network = private;
        let result = Broker::connect(
            a.clone(),
            Uuid::new_v4(),
            TaskProfile::Execute,
            Credentials(BTreeMap::new()),
            Arc::new(Policy(Mutex::new(a))),
            Arc::new(Audit::default()),
            limits(),
        )
        .await;
        assert_eq!(result.err(), Some(expected));
    }
    assert!(target.received_requests().await.unwrap().is_empty());
    let f = fixture("normal");
    let mut a = f.approval.clone();
    a.configuration.tools[0].name = "*".into();
    assert_eq!(
        Broker::connect(
            a.clone(),
            Uuid::new_v4(),
            TaskProfile::Execute,
            Credentials(BTreeMap::new()),
            Arc::new(Policy(Mutex::new(a))),
            Arc::new(Audit::default()),
            limits()
        )
        .await
        .err(),
        Some(BrokerError::Denied)
    );
    assert!(!f.pid.exists());
}

#[tokio::test]
async fn mcp_discovery_limits_and_initialization_cancellation() {
    for (mode, expected) in [
        ("duplicate", BrokerError::Protocol),
        ("pagination_loop", BrokerError::Protocol),
        ("init_hang", BrokerError::Timeout),
    ] {
        let f = fixture(mode);
        let mut l = limits();
        l.connect_timeout = if mode == "init_hang" {
            Duration::from_millis(500)
        } else {
            Duration::from_secs(3)
        };
        assert_eq!(
            Broker::connect(
                f.approval.clone(),
                Uuid::new_v4(),
                TaskProfile::Execute,
                Credentials(BTreeMap::new()),
                Arc::new(Policy(Mutex::new(f.approval.clone()))),
                Arc::new(Audit::default()),
                l
            )
            .await
            .err(),
            Some(expected)
        );
        stopped(&f).await;
    }
    let f = fixture("init_hang");
    let connect = Broker::connect(
        f.approval.clone(),
        Uuid::new_v4(),
        TaskProfile::Execute,
        Credentials(BTreeMap::new()),
        Arc::new(Policy(Mutex::new(f.approval.clone()))),
        Arc::new(Audit::default()),
        limits(),
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(500), connect)
            .await
            .is_err()
    );
    stopped(&f).await;
    let f = fixture("normal");
    let mut l = limits();
    l.max_tools = 1;
    assert_eq!(
        Broker::connect(
            f.approval.clone(),
            Uuid::new_v4(),
            TaskProfile::Execute,
            Credentials(BTreeMap::new()),
            Arc::new(Policy(Mutex::new(f.approval.clone()))),
            Arc::new(Audit::default()),
            l
        )
        .await
        .err(),
        Some(BrokerError::Oversized)
    );
    stopped(&f).await;
}

#[tokio::test]
async fn mcp_bounded_queue_and_input() {
    let f = fixture("hang");
    let mut l = limits();
    l.queue_capacity = 1;
    let (broker, access, _, _) = connect(f.approval.clone(), TaskProfile::Execute, l).await;
    assert_eq!(
        broker
            .tools_call(&access, "read", json!({"data": "x".repeat(8192)}))
            .await,
        Err(BrokerError::Oversized)
    );
    let results = futures_util::future::join_all(
        (0..8).map(|_| broker.tools_call(&access, "read", json!({}))),
    )
    .await;
    assert!(results.contains(&Err(BrokerError::Busy)));
    assert!(results.contains(&Err(BrokerError::Timeout)));
    stopped(&f).await;
}

#[tokio::test]
async fn mcp_profiles_and_empty_allowlist() {
    for profile in [
        TaskProfile::Review,
        TaskProfile::Research,
        TaskProfile::Delivery,
        TaskProfile::Execute,
    ] {
        let f = fixture("normal");
        let (broker, access, _, _) = connect(f.approval.clone(), profile, limits()).await;
        let result = broker.tools_call(&access, "write", json!({})).await;
        if profile == TaskProfile::Execute {
            assert!(result.is_ok());
        } else {
            assert_eq!(result, Err(BrokerError::Denied));
        }
        drop(broker);
        stopped(&f).await;
    }
    let mut f = fixture("normal");
    f.approval.configuration.tools.clear();
    let (broker, access, _, _) = connect(f.approval.clone(), TaskProfile::Execute, limits()).await;
    assert_eq!(
        broker.tools_list(&access).await.unwrap()["tools"],
        json!([])
    );
    assert_eq!(
        broker.tools_call(&access, "read", json!({})).await,
        Err(BrokerError::Denied)
    );
    drop(broker);
    stopped(&f).await;
}

#[tokio::test]
async fn mcp_unapproved_and_installer_commands_never_start() {
    let f = fixture("normal");
    for mode in [
        "disabled",
        "unapproved",
        "stale",
        "installer",
        "missing_credential",
    ] {
        let mut a = f.approval.clone();
        match mode {
            "disabled" => a.enabled = false,
            "unapproved" => a.approved_revision = None,
            "stale" => a.approved_revision = Some(2),
            "missing_credential" => {
                a.configuration
                    .credential_bindings
                    .insert("TOKEN".into(), "missing".into());
            }
            _ => {
                let path = f._dir.path().join("npx");
                std::fs::write(&path, "not an executable").unwrap();
                a.configuration.transport = McpTransport::Stdio {
                    executable: path.to_str().unwrap().into(),
                    arguments: vec![],
                };
            }
        }
        let result = Broker::connect(
            a.clone(),
            Uuid::new_v4(),
            TaskProfile::Execute,
            Credentials(BTreeMap::new()),
            Arc::new(Policy(Mutex::new(a))),
            Arc::new(Audit::default()),
            limits(),
        )
        .await;
        assert_eq!(result.err(), Some(BrokerError::Denied));
        assert!(!f.pid.exists());
    }
}

#[tokio::test]
async fn mcp_http_bounded_failure_responses() {
    for (template, expected) in [
        (
            ResponseTemplate::new(200).set_body_json(json!({"data": "x".repeat(8192)})),
            BrokerError::Oversized,
        ),
        (
            ResponseTemplate::new(200).set_body_raw("x".repeat(8192), "text/event-stream"),
            BrokerError::Oversized,
        ),
        (
            ResponseTemplate::new(200)
                .set_body_json(json!({}))
                .set_delay(Duration::from_secs(1)),
            BrokerError::Timeout,
        ),
        (
            ResponseTemplate::new(500).set_body_string("sensitive upstream error"),
            BrokerError::Upstream,
        ),
    ] {
        let server = MockServer::start().await;
        Mock::given(method("POST"))
            .respond_with(HttpFixture { sse: false })
            .mount(&server)
            .await;
        Mock::given(method("DELETE"))
            .respond_with(ResponseTemplate::new(204))
            .mount(&server)
            .await;
        let mut a = approval(McpTransport::StreamableHttp {
            endpoint: server.uri(),
        });
        a.allow_private_network = true;
        a.configuration
            .credential_bindings
            .insert("authorization".into(), "token".into());
        let (broker, access, _) = Broker::connect(
            a.clone(),
            Uuid::new_v4(),
            TaskProfile::Execute,
            Credentials(BTreeMap::from([(
                "token".into(),
                "Bearer fake-upstream-secret".into(),
            )])),
            Arc::new(Policy(Mutex::new(a))),
            Arc::new(Audit::default()),
            limits(),
        )
        .await
        .unwrap();
        Mock::given(method("POST"))
            .and(body_partial_json(json!({"method":"tools/call"})))
            .respond_with(template)
            .with_priority(1)
            .mount(&server)
            .await;
        let error = broker
            .tools_call(&access, "read", json!({}))
            .await
            .unwrap_err();
        assert_eq!(error, expected);
        assert!(!error.to_string().contains("sensitive"));
        let check = error.check_result(Uuid::new_v4(), 1);
        assert!(check.approved_tools.is_empty());
        assert_ne!(check.status, McpCheckStatus::Connected);
    }
}
