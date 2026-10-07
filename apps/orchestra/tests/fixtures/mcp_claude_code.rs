//! Binary-module regression tests (providers are intentionally not library exports).
use super::*;
use diraigent_orchestra::mcp::*;
use diraigent_types::{TaskProfile, mcp::*};
use std::{collections::BTreeMap, sync::Arc};

struct Policy(Approval);
#[async_trait]
impl Registry for Policy {
    async fn current(
        &self,
        project: uuid::Uuid,
        server: uuid::Uuid,
    ) -> Result<Approval, BrokerError> {
        assert_eq!(project, self.0.project_id);
        assert_eq!(server, self.0.server_id);
        Ok(self.0.clone())
    }
}
struct Audit;
impl AuditSink for Audit {
    fn record(&self, _: McpAuditEvent) {}
}

fn step() -> ResolvedTask {
    ResolvedTask {
        name: "test".into(),
        description: "test".into(),
        model: Some("model'quoted".into()),
        allowed_tools: None,
        allowed_tools_list: vec!["Read".into(), "Bash(git:*)".into()],
        budget: None,
        env: HashMap::new(),
        system_prompt: None,
        mcp_servers: None,
        agents: None,
        agent: Some("agent'quoted".into()),
        settings: Some(serde_json::json!({"language":"English"})),
    }
}

async fn sessions() -> (Sessions, tempfile::TempDir) {
    let python = std::env::split_paths(&std::env::var_os("PATH").unwrap())
        .map(|p| p.join("python3"))
        .find(|p| p.is_file())
        .unwrap()
        .canonicalize()
        .unwrap();
    let dir = tempfile::tempdir().unwrap();
    let approval = Approval {
        project_id: uuid::Uuid::new_v4(),
        server_id: uuid::Uuid::new_v4(),
        revision: 1,
        approved_revision: Some(1),
        enabled: true,
        allow_private_network: false,
        configuration: McpConfiguration {
            name: "fake".into(),
            transport: McpTransport::Stdio {
                executable: python.to_string_lossy().into(),
                arguments: vec![
                    format!(
                        "{}/tests/fixtures/mcp_server.py",
                        env!("CARGO_MANIFEST_DIR")
                    ),
                    "normal".into(),
                    dir.path().join("pid").to_string_lossy().into(),
                    dir.path().join("log").to_string_lossy().into(),
                ],
            },
            tools: vec![McpToolPermission {
                name: "read".into(),
                access: McpToolAccess::Read,
            }],
            credential_bindings: BTreeMap::new(),
        },
    };
    let id = approval.server_id;
    let (broker, access, _) = Broker::connect(
        approval.clone(),
        uuid::Uuid::new_v4(),
        TaskProfile::Review,
        Credentials(BTreeMap::from([(
            "secret".into(),
            "upstream-secret-never-exposed".into(),
        )])),
        Arc::new(Policy(approval)),
        Arc::new(Audit),
        Limits::default(),
    )
    .await
    .unwrap();
    (
        Sessions(vec![crate::engine::mcp::Connection::test_connection(
            id, broker, access,
        )]),
        dir,
    )
}

struct Fake {
    dir: tempfile::TempDir,
}
impl Fake {
    fn new(step: &mut ResolvedTask, mode: &str) -> Self {
        let dir = tempfile::Builder::new()
            .prefix("claude-test'quoted-")
            .tempdir()
            .unwrap();
        let python = std::env::split_paths(&std::env::var_os("PATH").unwrap())
            .map(|p| p.join("python3"))
            .find(|p| p.is_file())
            .unwrap();
        let script = format!("#!{}\n{}", python.display(), include_str!("claude_cli.py"));
        let path = dir.path().join("claude");
        std::fs::write(&path, script).unwrap();
        std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).unwrap();
        step.env.insert(
            "PATH".into(),
            format!(
                "{}:{}",
                dir.path().display(),
                std::env::var("PATH").unwrap()
            ),
        );
        step.env.insert(
            "FAKE_CAPTURE".into(),
            dir.path().join("capture.json").to_string_lossy().into(),
        );
        step.env.insert("FAKE_MODE".into(), mode.into());
        Self { dir }
    }
    fn capture(&self) -> Value {
        serde_json::from_slice(&std::fs::read(self.dir.path().join("capture.json")).unwrap())
            .unwrap()
    }
    async fn run(&self, step: &ResolvedTask) -> anyhow::Result<()> {
        let task = TaskContext {
            task_id: "task".into(),
            project_id: "project".into(),
            project_context: "prompt".into(),
            working_dir: Some(self.dir.path().into()),
            log_file: Some(self.dir.path().join("log")),
            user_prompt: None,
        };
        super::super::ProviderFactory::create("claude-code")
            .unwrap()
            .execute(
                step,
                &task,
                &ProviderConfig {
                    api_key: None,
                    base_url: None,
                    model: None,
                },
            )
            .await
            .map(|_| ())
    }
}

#[tokio::test]
async fn mcp_claude_managed_exact_config_and_cleanup() {
    let mut step = step();
    let (sessions, _fixture) = sessions().await;
    step.mcp_servers = Some(sessions);
    let fake = Fake::new(&mut step, "success");
    fake.run(&step).await.unwrap();
    let capture = fake.capture();
    let args = capture["args"].as_array().unwrap();
    assert_eq!(args.iter().filter(|v| **v == "--allowedTools").count(), 1);
    assert!(args.contains(&Value::from("--strict-mcp-config")));
    assert!(!args.contains(&Value::from("--dangerously-skip-permissions")));
    let mode = args.iter().position(|v| v == "--permission-mode").unwrap();
    assert_eq!(args[mode + 1], "dontAsk");
    let sources = args.iter().position(|v| v == "--setting-sources").unwrap();
    assert_eq!(args[sources + 1], "");
    assert!(args.contains(&Value::from("mcp__orchestra_0__read")));
    assert!(args.contains(&Value::from("model'quoted")));
    assert!(args.contains(&Value::from("agent'quoted")));
    assert_eq!(capture["file_mode"], 0o600);
    assert_eq!(capture["dir_mode"], 0o700);
    assert_eq!(capture["catalog"]["result"]["tools"][0]["name"], "read");
    assert!(capture["denied"]["error"].is_object());
    assert!(capture["allowed"]["result"].is_object());
    assert_eq!(capture["unauthenticated"], 401);
    assert!(!capture.to_string().contains("upstream-secret"));
    assert!(!Path::new(capture["runtime"].as_str().unwrap()).exists());
    step.mcp_servers.as_mut().unwrap().shutdown().await;
}

#[tokio::test]
async fn mcp_claude_failure_and_no_mcp_compatibility() {
    let mut step = step();
    let fake = Fake::new(&mut step, "success");
    fake.run(&step).await.unwrap();
    assert!(
        fake.capture()["args"]
            .as_array()
            .unwrap()
            .contains(&Value::from("--dangerously-skip-permissions"))
    );
    step.mcp_servers = Some(Sessions::default());
    step.env.insert("FAKE_MODE".into(), "empty".into());
    fake.run(&step).await.unwrap();
    assert!(
        fake.capture()["args"]
            .as_array()
            .unwrap()
            .contains(&Value::from("--strict-mcp-config"))
    );
    assert!(!Path::new(fake.capture()["runtime"].as_str().unwrap()).exists());
    let (sessions, _fixture) = sessions().await;
    step.mcp_servers = Some(sessions);
    step.env.insert("FAKE_MODE".into(), "failure".into());
    assert!(fake.run(&step).await.is_err());
    assert!(!Path::new(fake.capture()["runtime"].as_str().unwrap()).exists());
    step.env.insert("FAKE_MODE".into(), "unsupported".into());
    assert!(
        fake.run(&step)
            .await
            .unwrap_err()
            .to_string()
            .contains("strict MCP isolation")
    );
    step.env.insert("FAKE_MODE".into(), "old_version".into());
    assert!(
        fake.run(&step)
            .await
            .unwrap_err()
            .to_string()
            .contains(">= 2.1.246")
    );
    step.mcp_servers.as_mut().unwrap().shutdown().await;
}

#[tokio::test]
async fn mcp_claude_cancellation_cleans_runtime() {
    let mut step = step();
    let (sessions, _fixture) = sessions().await;
    step.mcp_servers = Some(sessions);
    let fake = Fake::new(&mut step, "sleep");
    let mut run = Box::pin(fake.run(&step));
    tokio::select! {
        result = &mut run => panic!("sleeping fake CLI exited before cancellation: {result:?}"),
        ready = tokio::time::timeout(std::time::Duration::from_secs(20), async {
            while std::fs::read(fake.dir.path().join("capture.json")).ok().and_then(|v| serde_json::from_slice::<Value>(&v).ok()).is_none() {
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
        }) => ready.expect("fake CLI must reach the running state"),
    }
    drop(run);
    assert!(!Path::new(fake.capture()["runtime"].as_str().unwrap()).exists());
    let pid = fake.capture()["pid"].as_i64().unwrap() as i32;
    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        while unsafe { libc::kill(pid, 0) } == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("cancelled Claude process must not survive");
    let endpoint = url::Url::parse(fake.capture()["endpoint"].as_str().unwrap()).unwrap();
    assert!(
        tokio::net::TcpStream::connect(("127.0.0.1", endpoint.port().unwrap()))
            .await
            .is_err()
    );
    step.mcp_servers.as_mut().unwrap().shutdown().await;
}

#[test]
fn mcp_claude_settings_cannot_widen_authority() {
    for settings in [
        serde_json::json!({"mcpServers":{}}),
        serde_json::json!({"permissions":{"defaultMode":"bypassPermissions"}}),
        serde_json::json!({"hooks":{}}),
        serde_json::json!({"env":{"CLAUDE_CODE_MCP": "yes"}}),
    ] {
        let mut step = step();
        step.settings = Some(settings);
        assert!(validate_managed_options(&step).is_err());
    }
    let mut step = step();
    step.allowed_tools_list.push("mcp__*".into());
    assert!(validate_managed_options(&step).is_err());
}

#[test]
fn mcp_claude_shell_values_are_literal() {
    let value = "quote' space $(printf injected); --mcp-config evil";
    let output = std::process::Command::new("bash")
        .arg("-c")
        .arg(format!("printf '%s' {}", quote(value)))
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(String::from_utf8(output.stdout).unwrap(), value);
}
