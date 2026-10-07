//! Authoritative API bridge and opaque, execution-owned connections.
use crate::{project::api::ProjectsApi, providers::ProviderFactory};
use async_trait::async_trait;
use diraigent_orchestra::mcp::*;
use diraigent_types::{TaskProfile, mcp::*};
use serde::Deserialize;
use serde_json::Value;
use std::{collections::BTreeSet, sync::Arc};
use uuid::Uuid;

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Selection {
    pub servers: Vec<ServerSelection>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ServerSelection {
    pub server_id: Uuid,
    pub tools: Option<BTreeSet<String>>,
}

pub fn selection(options: Option<&Value>) -> anyhow::Result<Option<Selection>> {
    if options.and_then(|v| v.get("mcp_servers")).is_some() {
        anyhow::bail!(
            "Legacy context.worker.mcp_servers is forbidden. Register and approve project MCP servers, then use context.worker.mcp with server_id/tool selections."
        );
    }
    // Settings are an opaque provider escape hatch, not a registry authority.
    if ["settings", "agents"]
        .iter()
        .any(|key| options.and_then(|v| v.get(*key)).is_some_and(has_raw_mcp))
    {
        anyhow::bail!(
            "Raw MCP settings/sub-agent definitions are forbidden; use approved project MCP servers."
        );
    }
    options.and_then(|v| v.get("mcp")).map(|v| {
        serde_json::from_value(v.clone()).map_err(|_| anyhow::anyhow!("Invalid MCP selection: only servers with server_id and optional tools are accepted"))
    }).transpose()
}

fn has_raw_mcp(v: &Value) -> bool {
    match v {
        Value::Object(o) => o
            .iter()
            .any(|(k, v)| k.to_ascii_lowercase().contains("mcp") || has_raw_mcp(v)),
        Value::Array(a) => a.iter().any(has_raw_mcp),
        _ => false,
    }
}

/// No Debug/Serialize/Clone: secrets and authority never enter prompts or scripts.
pub struct Connection {
    pub server_id: Uuid,
    broker: Broker,
    access: TaskAccess,
    tools: Option<BTreeSet<String>>,
}

impl Connection {
    #[cfg(test)]
    pub(crate) fn test_connection(server_id: Uuid, broker: Broker, access: TaskAccess) -> Self {
        Self {
            server_id,
            broker,
            access,
            tools: None,
        }
    }
    pub async fn tools_list(&self) -> Result<Value, BrokerError> {
        let mut result = self.broker.tools_list(&self.access).await?;
        if let Some(names) = &self.tools {
            result["tools"]
                .as_array_mut()
                .ok_or(BrokerError::Protocol)?
                .retain(|t| t["name"].as_str().is_some_and(|n| names.contains(n)));
        }
        Ok(result)
    }
    pub async fn tools_call(&self, name: &str, arguments: Value) -> Result<Value, BrokerError> {
        if self
            .tools
            .as_ref()
            .is_some_and(|tools| !tools.contains(name))
        {
            return Err(BrokerError::Denied);
        }
        self.broker.tools_call(&self.access, name, arguments).await
    }
}

#[derive(Default)]
pub struct Sessions(pub Vec<Connection>);
impl std::fmt::Debug for Sessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("McpSessions")
            .field("count", &self.0.len())
            .finish()
    }
}
impl Sessions {
    pub async fn shutdown(&mut self) {
        for connection in &mut self.0 {
            connection.broker.shutdown().await;
        }
    }
}

#[derive(Deserialize)]
struct Server {
    id: Uuid,
    project_id: Uuid,
    revision: i64,
    approved_revision: Option<i64>,
    approved_by: Option<Uuid>,
    enabled: bool,
    configuration: McpConfiguration,
}
impl Server {
    fn approval(self, project: Uuid) -> Result<Approval, BrokerError> {
        if self.project_id != project
            || !self.enabled
            || self.approved_by.is_none()
            || self.approved_revision != Some(self.revision)
        {
            return Err(BrokerError::Denied);
        }
        Ok(Approval {
            project_id: project,
            server_id: self.id,
            revision: self.revision,
            approved_revision: self.approved_revision,
            enabled: self.enabled,
            configuration: self.configuration,
            allow_private_network: false,
        })
    }
}
#[derive(Deserialize)]
struct Resolution {
    contract_version: u32,
    servers: Vec<Server>,
}

#[async_trait]
impl Registry for ProjectsApi {
    async fn current(&self, project: Uuid, server: Uuid) -> Result<Approval, BrokerError> {
        let value = self
            .mcp_get(&format!("/{project}/mcp-servers/{server}"))
            .await
            .map_err(|_| BrokerError::Stale)?;
        let current: Server = serde_json::from_value(value).map_err(|_| BrokerError::Protocol)?;
        if current.id != server {
            return Err(BrokerError::Denied);
        }
        // Revocation is fatal for an existing session, unlike denying an
        // individual unapproved tool name.
        current.approval(project).map_err(|_| BrokerError::Stale)
    }
}

struct Audit(ProjectsApi);
impl AuditSink for Audit {
    fn record(&self, event: McpAuditEvent) {
        let api = self.0.clone();
        tokio::spawn(async move {
            let _ = api
                .post_event(
                    &event.project_id.to_string(),
                    &serde_json::json!({
                        "kind": "custom", "title": "MCP operation", "source": "mcp",
                        "related_task_id": event.task_id, "metadata": event
                    }),
                )
                .await;
        });
    }
}

pub async fn resolve(
    api: &ProjectsApi,
    project: &str,
    task: &str,
    profile: TaskProfile,
    provider: &str,
    selection: Option<Selection>,
) -> anyhow::Result<Sessions> {
    let value = api
        .mcp_get(&format!("/{project}/mcp-servers/resolve"))
        .await?;
    let resolution: Resolution =
        serde_json::from_value(value).map_err(|_| BrokerError::Protocol)?;
    if resolution.contract_version != 1 {
        return Err(BrokerError::Protocol.into());
    }
    let mut servers = resolution.servers;
    if let Some(s) = &selection {
        let mut ids = BTreeSet::new();
        for requested in &s.servers {
            if !ids.insert(requested.server_id)
                || !servers.iter().any(|a| a.id == requested.server_id)
            {
                return Err(BrokerError::Denied.into());
            }
        }
        servers.retain(|a| ids.contains(&a.id));
    }
    if servers.is_empty() {
        return Ok(Sessions::default());
    }
    ProviderFactory::require_mcp(provider)?;
    let project = Uuid::parse_str(project).map_err(|_| BrokerError::Denied)?;
    let task = Uuid::parse_str(task).map_err(|_| BrokerError::Denied)?;
    let mut sessions = Sessions::default();
    for server in servers {
        let approval = server.approval(project)?;
        let tools = selection
            .as_ref()
            .and_then(|s| s.servers.iter().find(|s| s.server_id == approval.server_id))
            .and_then(|s| s.tools.clone());
        if tools.as_ref().is_some_and(|names| {
            names
                .iter()
                .any(|n| !approval.configuration.tools.iter().any(|t| &t.name == n))
        }) {
            return Err(BrokerError::Denied.into());
        }
        let credentials = api
            .mcp_post(
                &format!(
                    "/{project}/mcp-servers/{}/credentials/resolve",
                    approval.server_id
                ),
                &serde_json::json!({"revision": approval.revision}),
            )
            .await?;
        let credentials =
            Credentials(serde_json::from_value(credentials).map_err(|_| BrokerError::Protocol)?);
        let (broker, access, _) = Broker::connect(
            approval.clone(),
            task,
            profile,
            credentials,
            Arc::new(api.clone()),
            Arc::new(Audit(api.clone())),
            Limits::default(),
        )
        .await?;
        sessions.0.push(Connection {
            server_id: approval.server_id,
            broker,
            access,
            tools,
        });
    }
    Ok(sessions)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::engine::{local_source::LocalTaskSource, task_source::TaskSource};
    use serde_json::json;
    use wiremock::{
        Mock, MockServer, ResponseTemplate,
        matchers::{method, path},
    };

    #[test]
    fn mcp_task_json_cannot_define_or_approve_servers() {
        for raw in [
            json!({"mcp_servers": {"mcpServers": {"x": {"command": "evil", "token": "sentinel"}}}}),
            json!({"mcp": {"servers": [], "approved": true}}),
            json!({"mcp": {"servers": [{"server_id": Uuid::new_v4(), "executable": "evil"}]}}),
            json!({"settings": {"mcpServers": {"x": {"command": "evil"}}}}),
            json!({"agents": {"helper": {"mcpServers": {"x": {"command": "evil"}}}}}),
        ] {
            let error = selection(Some(&raw)).err().unwrap().to_string();
            assert!(!error.contains("sentinel"));
            assert!(!error.contains("evil"));
        }
        assert!(selection(None).unwrap().is_none());
        assert!(
            selection(Some(&json!({"mcp": {"servers": []}})))
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn mcp_local_source_cannot_self_approve() {
        let dir = tempfile::tempdir().unwrap();
        let work = dir.path().join("work.json");
        std::fs::write(
            &work,
            r#"{"title":"test","mcp":{"enabled":true,"approved":true}}"#,
        )
        .unwrap();
        let source = LocalTaskSource::from_file(&work, dir.path()).unwrap();
        assert!(
            source
                .resolve_mcp_sessions("p", "t", TaskProfile::Execute, "opencode", None)
                .await
                .unwrap()
                .0
                .is_empty()
        );
        let requested = Selection {
            servers: vec![ServerSelection {
                server_id: Uuid::new_v4(),
                tools: None,
            }],
        };
        assert!(
            source
                .resolve_mcp_sessions("p", "t", TaskProfile::Execute, "opencode", Some(requested))
                .await
                .is_err()
        );
    }

    #[test]
    fn mcp_runtime_secrets_are_not_prompt_context() {
        let mut context = json!({"project":{"metadata":{"mcp_servers":{"token":"sentinel"}}},
            "tasks":[{"context":{"spec":"keep this", "worker":{"env":{"TOKEN":"sentinel"}}}}],
            "credentials":{"secret":"sentinel"}});
        crate::engine::context::sanitize_prompt_context(&mut context);
        let text = serde_json::to_string(&context).unwrap();
        assert!(!text.contains("sentinel"));
        assert!(text.contains("keep this"));
    }

    struct Fixture {
        dir: tempfile::TempDir,
        server: Value,
        project: Uuid,
        id: Uuid,
        api: ProjectsApi,
        mock: MockServer,
    }
    impl Fixture {
        async fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let python = std::env::split_paths(&std::env::var_os("PATH").unwrap())
                .map(|p| p.join("python3"))
                .find(|p| p.is_file())
                .unwrap()
                .canonicalize()
                .unwrap();
            let project = Uuid::new_v4();
            let id = Uuid::new_v4();
            let server = json!({"id":id,"project_id":project,"revision":1,"approved_revision":1,
                "approved_by":Uuid::new_v4(),"enabled":true,"configuration":{
                    "name":"fixture","transport":{"transport":"stdio","executable":python,
                        "arguments":[std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/mcp_server.py"),
                            "normal",dir.path().join("pid"),dir.path().join("log")]},
                    "tools":[{"name":"read","access":"read"},{"name":"write","access":"write"}],
                    "credential_bindings":{"FIXTURE_TOKEN":"token"}}});
            let mock = MockServer::start().await;
            let api = ProjectsApi::new(&mock.uri(), "test-agent");
            Self {
                dir,
                server,
                project,
                id,
                api,
                mock,
            }
        }
        async fn mount(&self, value: Value) {
            Mock::given(method("GET"))
                .and(path(format!("/{}/mcp-servers/resolve", self.project)))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"contract_version":1,"servers":[value.clone()]})),
                )
                .mount(&self.mock)
                .await;
            Mock::given(method("GET"))
                .and(path(format!("/{}/mcp-servers/{}", self.project, self.id)))
                .respond_with(ResponseTemplate::new(200).set_body_json(value))
                .mount(&self.mock)
                .await;
            Mock::given(method("POST"))
                .and(path(format!(
                    "/{}/mcp-servers/{}/credentials/resolve",
                    self.project, self.id
                )))
                .respond_with(
                    ResponseTemplate::new(200)
                        .set_body_json(json!({"token":"fake-upstream-secret"})),
                )
                .mount(&self.mock)
                .await;
        }
        async fn resolve(
            &self,
            selection: Option<Selection>,
            provider: &str,
        ) -> anyhow::Result<Sessions> {
            resolve(
                &self.api,
                &self.project.to_string(),
                &Uuid::new_v4().to_string(),
                TaskProfile::Execute,
                provider,
                selection,
            )
            .await
        }
        async fn stopped(&self) {
            let pid: i32 = std::fs::read_to_string(self.dir.path().join("pid"))
                .unwrap()
                .parse()
                .unwrap();
            tokio::time::timeout(std::time::Duration::from_secs(3), async {
                while unsafe { libc::kill(pid, 0) } == 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(10)).await;
                }
            })
            .await
            .expect("task session must be reaped");
        }
    }

    #[tokio::test]
    async fn mcp_disabled_unapproved_and_wrong_project_never_start() {
        for field in ["enabled", "approved_revision", "approved_by", "project_id"] {
            let f = Fixture::new().await;
            let mut server = f.server.clone();
            server[field] = match field {
                "enabled" => json!(false),
                "project_id" => json!(Uuid::new_v4()),
                _ => Value::Null,
            };
            f.mount(server).await;
            assert!(f.resolve(None, "opencode").await.is_err());
            assert!(!f.dir.path().join("pid").exists());
        }
    }

    #[tokio::test]
    async fn mcp_selection_only_narrows_and_shutdown_reaps() {
        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        let select = Selection {
            servers: vec![ServerSelection {
                server_id: f.id,
                tools: Some(BTreeSet::from(["read".into()])),
            }],
        };
        let mut sessions = f.resolve(Some(select), "opencode").await.unwrap();
        assert_eq!(sessions.0[0].server_id, f.id);
        let tools = sessions.0[0].tools_list().await.unwrap();
        assert_eq!(tools["tools"].as_array().unwrap().len(), 1);
        assert_eq!(
            sessions.0[0].tools_call("write", json!({})).await,
            Err(BrokerError::Denied)
        );
        let result = sessions.0[0].tools_call("read", json!({})).await.unwrap();
        assert_eq!(result["credential_bound"], true);
        assert!(!format!("{sessions:?}").contains("fake-upstream-secret"));
        sessions.shutdown().await;
        f.stopped().await;
    }

    #[tokio::test]
    async fn mcp_unknown_tools_and_direct_providers_fail_without_starting() {
        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        for name in ["anthropic", "openai", "copilot", "ollama"] {
            let error = f.resolve(None, name).await.err().unwrap().to_string();
            assert!(error.contains("does not support MCP"));
        }
        let select = Selection {
            servers: vec![ServerSelection {
                server_id: f.id,
                tools: Some(BTreeSet::from(["unapproved".into()])),
            }],
        };
        assert!(f.resolve(Some(select), "opencode").await.is_err());
        assert!(!f.dir.path().join("pid").exists());
    }

    #[tokio::test]
    async fn mcp_provider_failure_and_cancellation_drop_sessions() {
        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        let sessions = f.resolve(None, "opencode").await.unwrap();
        let step = crate::providers::ResolvedTask {
            name: "test".into(),
            description: "test".into(),
            model: None,
            allowed_tools: None,
            allowed_tools_list: vec![],
            budget: None,
            env: Default::default(),
            system_prompt: None,
            mcp_servers: Some(sessions),
            agents: None,
            agent: None,
            settings: None,
        };
        let task = crate::providers::TaskContext {
            task_id: Uuid::new_v4().to_string(),
            project_id: f.project.to_string(),
            project_context: "{}".into(),
            working_dir: Some(f.dir.path().into()),
            log_file: None,
            user_prompt: None,
        };
        let cfg = crate::providers::ProviderConfig {
            api_key: None,
            base_url: None,
            model: None,
        };
        let error = ProviderFactory::create("opencode")
            .unwrap()
            .execute(&step, &task, &cfg)
            .await
            .unwrap_err()
            .to_string();
        assert!(error.contains("adapter is not implemented"));
        assert!(!error.contains("fake-upstream-secret"));
        assert!(!f.dir.path().join("mcp_config.json").exists());
        drop(step);
        f.stopped().await;

        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        let sessions = f.resolve(None, "opencode").await.unwrap();
        let (ready, started) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            let _sessions = sessions;
            let _ = ready.send(());
            std::future::pending::<()>().await;
        });
        started.await.unwrap();
        worker.abort();
        let _ = worker.await;
        f.stopped().await;
    }

    #[tokio::test]
    async fn mcp_registry_disable_invalidates_live_handle() {
        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        let sessions = f.resolve(None, "opencode").await.unwrap();
        let mut disabled = f.server.clone();
        disabled["enabled"] = json!(false);
        f.mock.reset().await;
        f.mount(disabled).await;
        assert_eq!(
            sessions.0[0].tools_call("read", json!({})).await,
            Err(BrokerError::Stale)
        );
        drop(sessions);
        f.stopped().await;
    }

    #[tokio::test]
    async fn mcp_revision_change_invalidates_and_closes_session() {
        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        let sessions = f.resolve(None, "opencode").await.unwrap();
        let mut changed = f.server.clone();
        changed["revision"] = json!(2);
        changed["approved_revision"] = json!(2);
        f.mock.reset().await;
        f.mount(changed).await;
        assert_eq!(sessions.0[0].tools_list().await, Err(BrokerError::Stale));
        f.stopped().await;
        assert_eq!(
            sessions.0[0].tools_list().await,
            Err(BrokerError::Disconnected)
        );
    }

    #[tokio::test]
    async fn mcp_empty_resolution_and_explicit_empty_selection_are_compatible() {
        let f = Fixture::new().await;
        f.mount(f.server.clone()).await;
        assert!(
            f.resolve(Some(Selection::default()), "openai")
                .await
                .unwrap()
                .0
                .is_empty()
        );
        assert!(!f.dir.path().join("pid").exists());
        f.mock.reset().await;
        Mock::given(method("GET"))
            .and(path(format!("/{}/mcp-servers/resolve", f.project)))
            .respond_with(
                ResponseTemplate::new(200)
                    .set_body_json(json!({"contract_version":1,"servers":[]})),
            )
            .mount(&f.mock)
            .await;
        assert!(f.resolve(None, "openai").await.unwrap().0.is_empty());
    }

    #[tokio::test]
    async fn mcp_upstream_errors_never_include_response_secrets() {
        let f = Fixture::new().await;
        Mock::given(method("GET"))
            .and(path(format!("/{}/mcp-servers/resolve", f.project)))
            .respond_with(ResponseTemplate::new(403).set_body_string("sentinel-upstream-token"))
            .mount(&f.mock)
            .await;
        let error = f.resolve(None, "opencode").await.err().unwrap().to_string();
        assert!(!error.contains("sentinel"));
        assert!(!error.contains(&f.mock.uri()));
    }

    #[tokio::test]
    async fn mcp_secrets_absent_from_full_and_trimmed_assembled_prompts() {
        let f = Fixture::new().await;
        let raw = json!({"project":{"metadata":{"mcp_servers":{"token":"sentinel"}}},
            "tasks":[{"context":{"worker":{"mcp_servers":{"env":{"TOKEN":"sentinel"}}}}}]});
        Mock::given(method("GET"))
            .and(path(format!("/agents/test-agent/context/{}", f.project)))
            .respond_with(ResponseTemplate::new(200).set_body_json(raw))
            .expect(2)
            .mount(&f.mock)
            .await;
        for mode in ["implement", "review"] {
            let prompt = crate::engine::prompt::build_user_prompt(
                &f.api,
                &Uuid::new_v4().to_string(),
                &f.project.to_string(),
                f.dir.path(),
                f.dir.path(),
                "agent-cli",
                mode,
                None,
                None,
            )
            .await;
            assert!(!prompt.contains("sentinel"));
            assert!(!prompt.contains("mcp_servers"));
        }
    }
}
