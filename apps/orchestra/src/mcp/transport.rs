use super::broker::{Approval, BrokerError, Credentials};
use diraigent_types::mcp::McpTransport;
use serde_json::{Value, json};
use std::{
    net::{IpAddr, SocketAddr},
    process::Stdio,
};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, BufReader},
    process::{Child, ChildStdin, ChildStdout, Command},
};

const VERSION: &str = "2025-06-18";

pub(super) struct Transport {
    inner: Upstream,
    max_bytes: usize,
    id: u64,
}
enum Upstream {
    Stdio(Process),
    Http {
        client: reqwest::Client,
        endpoint: url::Url,
        headers: reqwest::header::HeaderMap,
        session: Option<String>,
    },
}
struct Process {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
    group: Option<i32>,
}
impl Drop for Process {
    fn drop(&mut self) {
        if let Some(group) = self.group {
            // A dedicated process group contains the approved server and descendants.
            unsafe {
                libc::kill(-group, libc::SIGKILL);
            }
        }
        let _ = self.child.start_kill();
    }
}

impl Transport {
    pub(super) async fn connect(
        a: &Approval,
        credentials: Credentials,
        max_bytes: usize,
    ) -> Result<Self, BrokerError> {
        let bindings = &a.configuration.credential_bindings;
        let mut values = Vec::new();
        for (name, key) in bindings {
            let value = credentials.0.get(key).ok_or(BrokerError::Denied)?;
            values.push((name.as_str(), value.as_str()));
        }
        let inner = match &a.configuration.transport {
            McpTransport::Stdio {
                executable,
                arguments,
            } => {
                let path = std::path::Path::new(executable);
                if !path.is_absolute() || !path.is_file() {
                    return Err(BrokerError::Denied);
                }
                let canonical = path.canonicalize().map_err(|_| BrokerError::Denied)?;
                if [path, canonical.as_path()].iter().any(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                        matches!(
                            n,
                            "npx"
                                | "npm"
                                | "pnpm"
                                | "yarn"
                                | "bun"
                                | "uv"
                                | "uvx"
                                | "pip"
                                | "pip3"
                                | "cargo"
                                | "brew"
                                | "apt"
                                | "apt-get"
                        )
                    })
                }) {
                    return Err(BrokerError::Denied);
                }
                let mut command = Command::new(path);
                command
                    .args(arguments)
                    .env_clear()
                    .stdin(Stdio::piped())
                    .stdout(Stdio::piped())
                    .stderr(Stdio::null())
                    .kill_on_drop(true);
                // No shell, PATH, installer, ambient tokens, or inherited server settings.
                for (name, value) in values {
                    if name.is_empty()
                        || !name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_')
                        || name.as_bytes()[0].is_ascii_digit()
                    {
                        return Err(BrokerError::Denied);
                    }
                    command.env(name, value);
                }
                #[cfg(unix)]
                command.process_group(0);
                let mut child = command.spawn().map_err(|_| BrokerError::Upstream)?;
                let group = child.id().map(|pid| pid as i32);
                let stdin = child.stdin.take().ok_or(BrokerError::Disconnected)?;
                let stdout = BufReader::new(child.stdout.take().ok_or(BrokerError::Disconnected)?);
                Upstream::Stdio(Process {
                    child,
                    stdin,
                    stdout,
                    group,
                })
            }
            McpTransport::StreamableHttp { endpoint } => {
                let url = url::Url::parse(endpoint).map_err(|_| BrokerError::Denied)?;
                if !url.username().is_empty()
                    || url.password().is_some()
                    || url.query().is_some()
                    || url.fragment().is_some()
                    || (url.scheme() != "https"
                        && !(a.allow_private_network && url.scheme() == "http"))
                {
                    return Err(BrokerError::Denied);
                }
                let host = url.host_str().ok_or(BrokerError::Denied)?;
                let port = url.port_or_known_default().ok_or(BrokerError::Denied)?;
                let addresses: Vec<SocketAddr> = match url.host() {
                    Some(url::Host::Ipv4(ip)) => vec![SocketAddr::new(ip.into(), port)],
                    Some(url::Host::Ipv6(ip)) => vec![SocketAddr::new(ip.into(), port)],
                    _ => tokio::net::lookup_host((host, port))
                        .await
                        .map_err(|_| BrokerError::Upstream)?
                        .collect(),
                };
                if addresses.is_empty()
                    || (!a.allow_private_network && addresses.iter().any(|s| !public_ip(s.ip())))
                {
                    return Err(BrokerError::Denied);
                }
                // Pin validated DNS results; disable proxies and every redirect (including same-host).
                let client = reqwest::Client::builder()
                    .no_proxy()
                    .redirect(reqwest::redirect::Policy::none())
                    .resolve_to_addrs(host, &addresses)
                    .build()
                    .map_err(|_| BrokerError::Upstream)?;
                let mut headers = reqwest::header::HeaderMap::new();
                for (name, value) in values {
                    let name = reqwest::header::HeaderName::from_bytes(name.as_bytes())
                        .map_err(|_| BrokerError::Denied)?;
                    if matches!(
                        name.as_str(),
                        "host"
                            | "content-type"
                            | "content-length"
                            | "accept"
                            | "connection"
                            | "transfer-encoding"
                            | "mcp-session-id"
                            | "mcp-protocol-version"
                    ) {
                        return Err(BrokerError::Denied);
                    }
                    let mut value = reqwest::header::HeaderValue::from_str(value)
                        .map_err(|_| BrokerError::Denied)?;
                    value.set_sensitive(true);
                    headers.insert(name, value);
                }
                Upstream::Http {
                    client,
                    endpoint: url,
                    headers,
                    session: None,
                }
            }
        };
        Ok(Self {
            inner,
            max_bytes,
            id: 0,
        })
    }

    pub(super) async fn initialize(&mut self) -> Result<(), BrokerError> {
        let result = self
            .rpc(
                "initialize",
                json!({"protocolVersion": VERSION, "capabilities": {},
            "clientInfo": {"name": "diraigent-broker", "version": "1"}}),
            )
            .await?;
        if !matches!(
            result.get("protocolVersion").and_then(Value::as_str),
            Some("2025-06-18")
        ) || !result
            .get("capabilities")
            .and_then(|c| c.get("tools"))
            .is_some_and(Value::is_object)
        {
            return Err(BrokerError::Protocol);
        }
        self.exchange(
            json!({"jsonrpc":"2.0", "method":"notifications/initialized"}),
            None,
        )
        .await?;
        Ok(())
    }

    pub(super) async fn rpc(&mut self, method: &str, params: Value) -> Result<Value, BrokerError> {
        self.id += 1;
        self.exchange(
            json!({"jsonrpc":"2.0", "id":self.id, "method":method, "params":params}),
            Some(self.id),
        )
        .await
    }

    async fn exchange(&mut self, message: Value, id: Option<u64>) -> Result<Value, BrokerError> {
        let encoded = serde_json::to_vec(&message).map_err(|_| BrokerError::Protocol)?;
        if encoded.len() > self.max_bytes {
            return Err(BrokerError::Oversized);
        }
        match &mut self.inner {
            Upstream::Stdio(process) => {
                process
                    .stdin
                    .write_all(&encoded)
                    .await
                    .map_err(|_| BrokerError::Disconnected)?;
                process
                    .stdin
                    .write_all(b"\n")
                    .await
                    .map_err(|_| BrokerError::Disconnected)?;
                process
                    .stdin
                    .flush()
                    .await
                    .map_err(|_| BrokerError::Disconnected)?;
                let Some(id) = id else {
                    return Ok(Value::Null);
                };
                let mut total = 0;
                loop {
                    let mut line = Vec::new();
                    let mut byte = [0];
                    loop {
                        if process
                            .stdout
                            .read(&mut byte)
                            .await
                            .map_err(|_| BrokerError::Disconnected)?
                            == 0
                        {
                            return Err(BrokerError::Disconnected);
                        }
                        total += 1;
                        if total > self.max_bytes {
                            return Err(BrokerError::Oversized);
                        }
                        if byte[0] == b'\n' {
                            break;
                        }
                        line.push(byte[0]);
                    }
                    if let Some(result) = response(&line, id)? {
                        return Ok(result);
                    }
                }
            }
            Upstream::Http {
                client,
                endpoint,
                headers,
                session,
            } => {
                let mut request = client
                    .post(endpoint.clone())
                    .headers(headers.clone())
                    .header("accept", "application/json, text/event-stream")
                    .header("content-type", "application/json")
                    .header("mcp-protocol-version", VERSION)
                    .body(encoded);
                if let Some(session) = session.as_ref() {
                    request = request.header("mcp-session-id", session);
                }
                let mut reply = request
                    .send()
                    .await
                    .map_err(|_| BrokerError::Disconnected)?;
                if !reply.status().is_success() {
                    return Err(BrokerError::Upstream);
                }
                if let Some(value) = reply.headers().get("mcp-session-id") {
                    let value = value.to_str().map_err(|_| BrokerError::Protocol)?;
                    if value.is_empty()
                        || value.len() > 128
                        || !value.bytes().all(|b| (0x21..=0x7e).contains(&b))
                    {
                        return Err(BrokerError::Protocol);
                    }
                    if session.as_ref().is_some_and(|s| s != value) {
                        return Err(BrokerError::Protocol);
                    }
                    *session = Some(value.to_owned());
                }
                let Some(id) = id else {
                    if reply.status() != reqwest::StatusCode::ACCEPTED
                        && reply.status() != reqwest::StatusCode::NO_CONTENT
                    {
                        return Err(BrokerError::Protocol);
                    }
                    return Ok(Value::Null);
                };
                let content_type = reply
                    .headers()
                    .get("content-type")
                    .and_then(|v| v.to_str().ok())
                    .unwrap_or("");
                let sse = content_type.split(';').next() == Some("text/event-stream");
                if !sse && content_type.split(';').next() != Some("application/json") {
                    return Err(BrokerError::Protocol);
                }
                if reply
                    .content_length()
                    .is_some_and(|n| n > self.max_bytes as u64)
                {
                    return Err(BrokerError::Oversized);
                }
                let mut buffer = Vec::new();
                let mut total = 0;
                while let Some(chunk) =
                    reply.chunk().await.map_err(|_| BrokerError::Disconnected)?
                {
                    total += chunk.len();
                    if total > self.max_bytes {
                        return Err(BrokerError::Oversized);
                    }
                    buffer.extend_from_slice(&chunk);
                    if sse {
                        while let Some((end, delimiter)) = event_end(&buffer) {
                            let event = std::str::from_utf8(&buffer[..end])
                                .map_err(|_| BrokerError::Protocol)?;
                            let data = event
                                .split(['\r', '\n'])
                                .filter_map(|l| {
                                    l.strip_prefix("data:")
                                        .map(|s| s.strip_prefix(' ').unwrap_or(s))
                                })
                                .collect::<Vec<_>>()
                                .join("\n");
                            if !data.is_empty()
                                && let Some(result) = response(data.as_bytes(), id)?
                            {
                                return Ok(result);
                            }
                            buffer.drain(..end + delimiter);
                        }
                    }
                }
                if sse {
                    Err(BrokerError::Disconnected)
                } else {
                    response(&buffer, id)?.ok_or(BrokerError::Protocol)
                }
            }
        }
    }

    pub(super) async fn close(&mut self) {
        match &mut self.inner {
            Upstream::Stdio(process) => {
                if let Some(group) = process.group.take() {
                    unsafe {
                        libc::kill(-group, libc::SIGKILL);
                    }
                }
                let _ = process.child.start_kill();
                let _ =
                    tokio::time::timeout(std::time::Duration::from_secs(1), process.child.wait())
                        .await;
            }
            Upstream::Http {
                client,
                endpoint,
                headers,
                session: Some(session),
            } => {
                let _ = tokio::time::timeout(
                    std::time::Duration::from_secs(1),
                    client
                        .delete(endpoint.clone())
                        .headers(headers.clone())
                        .header("mcp-session-id", session.as_str())
                        .header("mcp-protocol-version", VERSION)
                        .send(),
                )
                .await;
            }
            _ => {}
        }
    }

    /// Reap spontaneous subprocess exits promptly even while no tools are being called.
    pub(super) async fn exited(&mut self) {
        match &mut self.inner {
            Upstream::Stdio(process) => {
                let _ = process.child.wait().await;
            }
            Upstream::Http { .. } => std::future::pending::<()>().await,
        }
    }
}

fn response(bytes: &[u8], id: u64) -> Result<Option<Value>, BrokerError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| BrokerError::Protocol)?;
    if value.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(BrokerError::Protocol);
    }
    // Ignore notifications only; deny server requests (sampling, elicitation, roots).
    if value.get("method").is_some() {
        return if value.get("id").is_none() {
            Ok(None)
        } else {
            Err(BrokerError::Protocol)
        };
    }
    if value.get("id").and_then(Value::as_u64) != Some(id) {
        return Err(BrokerError::Protocol);
    }
    if value.get("error").is_some() {
        return Err(BrokerError::Upstream);
    }
    value
        .get("result")
        .cloned()
        .map(Some)
        .ok_or(BrokerError::Protocol)
}

fn event_end(buffer: &[u8]) -> Option<(usize, usize)> {
    [
        b"\n\n".as_slice(),
        b"\r\r",
        b"\r\n\r\n",
        b"\r\n\n",
        b"\n\r\n",
    ]
    .iter()
    .filter_map(|delimiter| {
        buffer
            .windows(delimiter.len())
            .position(|w| w == *delimiter)
            .map(|p| (p, delimiter.len()))
    })
    .min_by_key(|(p, _)| *p)
}

fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => {
            let [a, b, c, _] = ip.octets();
            !(ip.is_private()
                || ip.is_loopback()
                || ip.is_link_local()
                || ip.is_multicast()
                || ip.is_unspecified()
                || ip.is_broadcast()
                || a == 0
                || a >= 240
                || (a == 100 && (64..=127).contains(&b))
                || (a == 192 && b == 0)
                || (a == 192 && b == 88 && c == 99)
                || (a == 198 && (b == 18 || b == 19 || (b == 51 && c == 100)))
                || (a == 203 && b == 0 && c == 113))
        }
        IpAddr::V6(ip) => {
            if let Some(ip) = ip.to_ipv4_mapped() {
                return public_ip(ip.into());
            }
            let segments = ip.segments();
            // Only global unicast; deny documentation, translation and transition ranges.
            (segments[0] & 0xe000) == 0x2000
                && segments[0] != 0x3fff
                && segments[0] != 0x2002
                && !(segments[0] == 0x2001 && (segments[1] < 0x200 || segments[1] == 0xdb8))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mcp_sse_framing_selects_earliest_event() {
        assert_eq!(
            event_end(b"data: first\r\n\r\ndata: second\n\n"),
            Some((11, 4))
        );
        assert_eq!(event_end(b"data: first\r\r"), Some((11, 2)));
        assert_eq!(event_end(b"data: first\n"), None);
    }

    #[test]
    fn mcp_endpoint_address_policy() {
        for address in [
            "127.0.0.1",
            "10.0.0.1",
            "172.16.0.1",
            "192.168.1.1",
            "169.254.169.254",
            "100.64.0.1",
            "192.0.2.1",
            "198.51.100.1",
            "203.0.113.1",
            "224.0.0.1",
            "240.0.0.1",
            "::",
            "::1",
            "::ffff:127.0.0.1",
            "fc00::1",
            "fe80::1",
            "2001:db8::1",
            "2002::1",
            "3fff::1",
        ] {
            assert!(!public_ip(address.parse().unwrap()), "{address}");
        }
        assert!(public_ip("8.8.8.8".parse().unwrap()));
        assert!(public_ip("2606:4700:4700::1111".parse().unwrap()));
    }
}
