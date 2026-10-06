use super::transport::Transport;
use async_trait::async_trait;
use diraigent_types::{mcp::*, task_profile::TaskProfile};
use serde_json::{Value, json};
use std::{collections::BTreeMap, sync::Arc, time::Duration};
use tokio::sync::{mpsc, oneshot};
use uuid::Uuid;

/// Supplied only by authenticated registry resolution, never task JSON or discovery.
#[derive(Clone)]
pub struct Approval {
    pub project_id: Uuid,
    pub server_id: Uuid,
    pub revision: i64,
    pub approved_revision: Option<i64>,
    pub enabled: bool,
    pub configuration: McpConfiguration,
    /// Separate, explicit human endpoint approval. Default must be false.
    pub allow_private_network: bool,
}

/// Bridge implementations must fetch current authoritative approval on every operation.
#[async_trait]
pub trait Registry: Send + Sync {
    async fn current(&self, project: Uuid, server: Uuid) -> Result<Approval, BrokerError>;
}

/// Intentionally neither Debug nor Serialize. Values remain upstream only.
pub struct Credentials(pub BTreeMap<String, String>);

impl Drop for Credentials {
    fn drop(&mut self) {
        use zeroize::Zeroize;
        for value in self.0.values_mut() {
            value.zeroize();
        }
    }
}

#[derive(Clone)]
pub struct Limits {
    pub connect_timeout: Duration,
    pub call_timeout: Duration,
    pub lifetime: Duration,
    pub max_bytes: usize,
    pub max_tools: usize,
    pub queue_capacity: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
            call_timeout: Duration::from_secs(30),
            lifetime: Duration::from_secs(1800),
            max_bytes: 1024 * 1024,
            max_tools: 256,
            queue_capacity: 8,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum BrokerError {
    #[error("MCP policy denied")]
    Denied,
    #[error("MCP approval changed")]
    Stale,
    #[error("MCP operation timed out")]
    Timeout,
    #[error("MCP connection closed")]
    Disconnected,
    #[error("MCP response exceeds limit")]
    Oversized,
    #[error("MCP protocol error")]
    Protocol,
    #[error("MCP upstream failed")]
    Upstream,
    #[error("MCP broker busy")]
    Busy,
}

/// Opaque capability, tied to the broker's project, task and execution profile.
/// Not serializable/debuggable: future adapters must use a private authenticated
/// channel, not publish this alongside an unauthenticated localhost listener.
pub struct TaskAccess {
    secret: Uuid,
    project: Uuid,
    task: Uuid,
}

pub trait AuditSink: Send + Sync {
    /// Must be nonblocking; no arguments, results, endpoints or credentials here.
    fn record(&self, event: McpAuditEvent);
}

struct Request {
    operation: Operation,
    reply: oneshot::Sender<Result<Value, BrokerError>>,
}
enum Operation {
    List,
    Call { name: String, arguments: Value },
}

pub struct Broker {
    shutdown: Option<oneshot::Sender<()>>,
    runner: Option<tokio::task::JoinHandle<()>>,
    sender: mpsc::Sender<Request>,
    secret: Uuid,
    project: Uuid,
    task: Uuid,
    max_bytes: usize,
    call_timeout: Duration,
}

impl Broker {
    /// Starts one bounded upstream session. A dropped connect future drops and
    /// kills its subprocess. A dropped operation future closes the session, so
    /// interrupted stdio replies can never be consumed by a later request.
    #[allow(clippy::too_many_arguments)]
    pub async fn connect(
        approval: Approval,
        task: Uuid,
        profile: TaskProfile,
        credentials: Credentials,
        registry: Arc<dyn Registry>,
        audit: Arc<dyn AuditSink>,
        limits: Limits,
    ) -> Result<(Self, TaskAccess, McpCheckResult), BrokerError> {
        validate(&approval)?;
        if limits.max_bytes == 0
            || limits.max_tools == 0
            || limits.queue_capacity == 0
            || limits.max_bytes > 16 * 1024 * 1024
            || limits.max_tools > 4096
            || limits.queue_capacity > 64
            || limits.connect_timeout.is_zero()
            || limits.call_timeout.is_zero()
            || limits.lifetime.is_zero()
            || limits.connect_timeout > Duration::from_secs(60)
            || limits.call_timeout > Duration::from_secs(300)
            || limits.lifetime > Duration::from_secs(3600)
        {
            return Err(BrokerError::Denied);
        }
        let mut transport = tokio::time::timeout(limits.connect_timeout, async {
            recheck(&*registry, &approval).await?;
            let mut transport =
                Transport::connect(&approval, credentials, limits.max_bytes).await?;
            transport.initialize().await?;
            Ok::<_, BrokerError>(transport)
        })
        .await
        .map_err(|_| BrokerError::Timeout)??;
        tokio::time::timeout(limits.connect_timeout, recheck(&*registry, &approval))
            .await
            .map_err(|_| BrokerError::Timeout)??;
        let discovered = tokio::time::timeout(
            limits.connect_timeout,
            discover(&mut transport, &approval, profile, &limits),
        )
        .await
        .map_err(|_| BrokerError::Timeout)??;
        tokio::time::timeout(limits.connect_timeout, recheck(&*registry, &approval))
            .await
            .map_err(|_| BrokerError::Timeout)??;
        let check = McpCheckResult {
            server_id: approval.server_id,
            revision: approval.revision,
            status: McpCheckStatus::Connected,
            approved_tools: discovered.keys().cloned().collect(),
        };
        let project = approval.project_id;
        let secret = Uuid::new_v4();
        let (sender, receiver) = mpsc::channel(limits.queue_capacity);
        let max_bytes = limits.max_bytes;
        let call_timeout = limits.call_timeout;
        let (shutdown, cancelled) = oneshot::channel();
        let runner = tokio::spawn(run(
            transport, receiver, approval, profile, registry, audit, limits, discovered, task,
            cancelled,
        ));
        Ok((
            Self {
                shutdown: Some(shutdown),
                runner: Some(runner),
                sender,
                secret,
                project,
                task,
                max_bytes,
                call_timeout,
            },
            TaskAccess {
                secret,
                project,
                task,
            },
            check,
        ))
    }

    pub async fn tools_list(&self, access: &TaskAccess) -> Result<Value, BrokerError> {
        self.request(access, Operation::List).await
    }

    pub async fn shutdown(&mut self) {
        self.shutdown.take();
        if let Some(runner) = self.runner.take() {
            let _ = runner.await;
        }
    }

    pub async fn tools_call(
        &self,
        access: &TaskAccess,
        name: &str,
        arguments: Value,
    ) -> Result<Value, BrokerError> {
        if name.len() > 128 {
            return Err(BrokerError::Denied);
        }
        self.request(
            access,
            Operation::Call {
                name: name.to_owned(),
                arguments,
            },
        )
        .await
    }

    async fn request(
        &self,
        access: &TaskAccess,
        operation: Operation,
    ) -> Result<Value, BrokerError> {
        if access.secret != self.secret
            || access.project != self.project
            || access.task != self.task
        {
            return Err(BrokerError::Denied);
        }
        if let Operation::Call { name, arguments } = &operation {
            if name.len() > 128 || !arguments.is_object() {
                return Err(BrokerError::Denied);
            }
            // Bound queued input without first allocating its serialized representation.
            let mut counter = ByteBudget(self.max_bytes.saturating_sub(name.len() + 128));
            serde_json::to_writer(&mut counter, arguments).map_err(|_| BrokerError::Oversized)?;
        }
        let (reply, result) = oneshot::channel();
        self.sender
            .try_send(Request { operation, reply })
            .map_err(|error| match error {
                mpsc::error::TrySendError::Full(_) => BrokerError::Busy,
                mpsc::error::TrySendError::Closed(_) => BrokerError::Disconnected,
            })?;
        tokio::time::timeout(self.call_timeout, result)
            .await
            .map_err(|_| BrokerError::Timeout)?
            .map_err(|_| BrokerError::Disconnected)?
    }
}

impl Drop for Broker {
    fn drop(&mut self) {
        // Wake even an in-flight operation; the actor closes the upstream.
        self.shutdown.take();
    }
}

struct ByteBudget(usize);
impl std::io::Write for ByteBudget {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_sub(bytes.len())
            .ok_or_else(|| std::io::Error::other("size limit"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl BrokerError {
    /// The API bridge can report failed checks without retaining an upstream error.
    pub fn check_result(self, server_id: Uuid, revision: i64) -> McpCheckResult {
        McpCheckResult {
            server_id,
            revision,
            approved_tools: Vec::new(),
            status: match self {
                Self::Denied | Self::Stale => McpCheckStatus::Denied,
                Self::Timeout => McpCheckStatus::TimedOut,
                _ => McpCheckStatus::Failed,
            },
        }
    }
}

fn validate(a: &Approval) -> Result<(), BrokerError> {
    if !a.enabled || a.revision < 1 || a.approved_revision != Some(a.revision) {
        return Err(BrokerError::Denied);
    }
    if a.configuration.tools.len() > 256 || a.configuration.credential_bindings.len() > 128 {
        return Err(BrokerError::Denied);
    }
    let mut names = std::collections::BTreeSet::new();
    for tool in &a.configuration.tools {
        if tool.name.is_empty()
            || tool.name.len() > 128
            || !tool
                .name
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_-.".contains(&b))
            || !names.insert(&tool.name)
        {
            return Err(BrokerError::Denied);
        }
    }
    Ok(())
}

async fn recheck(registry: &dyn Registry, pinned: &Approval) -> Result<(), BrokerError> {
    let current = registry
        .current(pinned.project_id, pinned.server_id)
        .await?;
    validate(&current)?;
    if current.project_id != pinned.project_id
        || current.server_id != pinned.server_id
        || current.revision != pinned.revision
        || current.configuration != pinned.configuration
        || current.allow_private_network != pinned.allow_private_network
    {
        return Err(BrokerError::Stale);
    }
    Ok(())
}

fn allowed(a: &Approval, profile: TaskProfile, name: &str) -> bool {
    a.configuration.tools.iter().any(|t| {
        t.name == name && (profile == TaskProfile::Execute || t.access == McpToolAccess::Read)
    })
}

async fn discover(
    transport: &mut Transport,
    approval: &Approval,
    profile: TaskProfile,
    limits: &Limits,
) -> Result<BTreeMap<String, Value>, BrokerError> {
    let mut tools = BTreeMap::new();
    let mut all_names = std::collections::BTreeSet::new();
    let mut cursor = None;
    let mut cursors = std::collections::BTreeSet::new();
    let mut bytes = 0;
    loop {
        let result = transport
            .rpc(
                "tools/list",
                match &cursor {
                    Some(cursor) => json!({"cursor": cursor}),
                    None => json!({}),
                },
            )
            .await?;
        bytes += serde_json::to_vec(&result)
            .map_err(|_| BrokerError::Protocol)?
            .len();
        if bytes > limits.max_bytes {
            return Err(BrokerError::Oversized);
        }
        let page = result
            .get("tools")
            .and_then(Value::as_array)
            .ok_or(BrokerError::Protocol)?;
        for tool in page {
            let name = tool
                .get("name")
                .and_then(Value::as_str)
                .ok_or(BrokerError::Protocol)?;
            if !all_names.insert(name.to_owned()) {
                return Err(BrokerError::Protocol);
            }
            if all_names.len() > limits.max_tools {
                return Err(BrokerError::Oversized);
            }
            if allowed(approval, profile, name) {
                if !tool.get("inputSchema").is_some_and(Value::is_object) {
                    return Err(BrokerError::Protocol);
                }
                tools.insert(name.to_owned(), tool.clone());
            }
        }
        cursor = match result.get("nextCursor") {
            None | Some(Value::Null) => break,
            Some(Value::String(s)) if !s.is_empty() => Some(s.clone()),
            _ => return Err(BrokerError::Protocol),
        };
        if !cursors.insert(cursor.clone()) || cursors.len() > limits.max_tools {
            return Err(BrokerError::Protocol);
        }
    }
    Ok(tools)
}

#[allow(clippy::too_many_arguments)]
async fn run(
    mut transport: Transport,
    mut receiver: mpsc::Receiver<Request>,
    approval: Approval,
    profile: TaskProfile,
    registry: Arc<dyn Registry>,
    audit: Arc<dyn AuditSink>,
    limits: Limits,
    discovered: BTreeMap<String, Value>,
    task: Uuid,
    mut cancelled: oneshot::Receiver<()>,
) {
    let deadline = tokio::time::Instant::now() + limits.lifetime;
    loop {
        let request = tokio::select! {
            _ = &mut cancelled => break,
            r = receiver.recv() => match r { Some(r) => r, None => break },
            _ = tokio::time::sleep_until(deadline) => break,
            _ = transport.exited() => break,
        };
        let Request {
            operation,
            mut reply,
        } = request;
        if reply.is_closed() {
            continue;
        }
        let tool_name = match &operation {
            Operation::List => None,
            Operation::Call { name, .. } =>
            // Never audit attacker-controlled unknown names.
            {
                approval
                    .configuration
                    .tools
                    .iter()
                    .find(|t| t.name == *name)
                    .map(|t| t.name.clone())
            }
        };
        let result = tokio::select! {
            _ = &mut cancelled => {
                audit.record(McpAuditEvent { project_id: approval.project_id, task_id: task,
                    server_id: approval.server_id, revision: approval.revision, tool_name,
                    outcome: McpAuditOutcome::Cancelled });
                break;
            },
            _ = reply.closed() => {
                audit.record(McpAuditEvent { project_id: approval.project_id, task_id: task,
                    server_id: approval.server_id, revision: approval.revision, tool_name,
                    outcome: McpAuditOutcome::Cancelled });
                break;
            },
            _ = tokio::time::sleep_until(deadline) => Err(BrokerError::Timeout),
            r = tokio::time::timeout(limits.call_timeout, async {
                recheck(&*registry, &approval).await?;
                match operation {
                    Operation::List => {
                        let fresh = discover(&mut transport, &approval, profile, &limits).await?;
                        if fresh != discovered { return Err(BrokerError::Stale); }
                        recheck(&*registry, &approval).await?;
                        Ok(json!({"tools": discovered.values().collect::<Vec<_>>()}))
                    }
                    Operation::Call { name, arguments } => {
                        if !allowed(&approval, profile, &name) || !discovered.contains_key(&name) {
                            return Err(BrokerError::Denied);
                        }
                        // Rediscover and require identical approved tool definition. A changed
                        // schema/description/annotation needs a new human-approved session.
                        let fresh = discover(&mut transport, &approval, profile, &limits).await?;
                        if fresh.get(&name) != discovered.get(&name) { return Err(BrokerError::Stale); }
                        recheck(&*registry, &approval).await?;
                        let result = transport.rpc("tools/call", json!({"name": name, "arguments": arguments})).await?;
                        if !result.get("content").is_some_and(Value::is_array)
                            || result.get("isError").is_some_and(|v| !v.is_boolean()) {
                            return Err(BrokerError::Protocol);
                        }
                        Ok(result)
                    }
                }
            }) => r.unwrap_or(Err(BrokerError::Timeout)),
        };
        audit.record(McpAuditEvent {
            project_id: approval.project_id,
            task_id: task,
            server_id: approval.server_id,
            revision: approval.revision,
            tool_name,
            outcome: match result {
                Ok(_) => McpAuditOutcome::Succeeded,
                Err(BrokerError::Denied | BrokerError::Stale) => McpAuditOutcome::Denied,
                Err(BrokerError::Timeout) => McpAuditOutcome::TimedOut,
                _ => McpAuditOutcome::Failed,
            },
        });
        let fatal = result
            .as_ref()
            .err()
            .is_some_and(|e| *e != BrokerError::Denied);
        let _ = reply.send(result);
        if fatal {
            break;
        }
    }
    transport.close().await;
}
