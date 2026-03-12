use crate::api::ProjectsApi;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Mutex;
use tracing::{info, warn};

/// Cooldown before re-reporting the same fingerprint (1 hour).
const COOLDOWN_SECS: u64 = 3600;
/// Polling interval for each project's log check.
const CHECK_INTERVAL_SECS: u64 = 120;
/// Max log entries to fetch per query.
const QUERY_LIMIT: u32 = 100;

struct ProjectState {
    last_checked: chrono::DateTime<chrono::Utc>,
    /// fingerprint → last-seen timestamp
    seen: HashMap<String, std::time::Instant>,
}

/// Spawn log monitor tasks for all projects that have both `loki_url` (global)
/// and `service_name` (per project).
pub async fn spawn_log_monitors(
    api: &ProjectsApi,
    loki_url: String,
    loki_label: String,
    shutdown: Arc<AtomicBool>,
) {
    let projects = match api.list_projects().await {
        Ok(p) => p,
        Err(e) => {
            warn!("log_monitor: failed to list projects: {e}");
            return;
        }
    };

    for project in &projects {
        let service_name = match project["service_name"].as_str() {
            Some(s) if !s.is_empty() => s.to_string(),
            _ => continue,
        };
        let project_id = match project["id"].as_str() {
            Some(id) => id.to_string(),
            None => continue,
        };
        let project_name = project["name"].as_str().unwrap_or("?");

        info!(
            "log_monitor: starting for project \"{project_name}\" (service={service_name}, label={loki_label})"
        );

        let api = api.clone();
        let loki = loki_url.clone();
        let label = loki_label.clone();
        let shutdown = shutdown.clone();

        tokio::spawn(async move {
            run_monitor(api, loki, label, project_id, service_name, shutdown).await;
        });
    }
}

async fn run_monitor(
    api: ProjectsApi,
    loki_url: String,
    loki_label: String,
    project_id: String,
    service_name: String,
    shutdown: Arc<AtomicBool>,
) {
    let state = Arc::new(Mutex::new(ProjectState {
        last_checked: chrono::Utc::now(),
        seen: HashMap::new(),
    }));

    let mut interval = tokio::time::interval(std::time::Duration::from_secs(CHECK_INTERVAL_SECS));
    // First tick fires immediately — skip it so we don't query from epoch
    interval.tick().await;

    loop {
        tokio::select! {
            _ = interval.tick() => {}
            _ = crate::util::wait_shutdown(&shutdown) => { return; }
        }

        if shutdown.load(Ordering::SeqCst) {
            return;
        }

        // Heartbeat is handled by main loop, but we contribute to liveness
        // by simply continuing to run.

        if let Err(e) = check_logs(
            &api,
            &loki_url,
            &loki_label,
            &project_id,
            &service_name,
            &state,
        )
        .await
        {
            warn!("log_monitor[{service_name}]: {e}");
            // Survives Loki being unavailable — just log and retry next tick
        }
    }
}

async fn check_logs(
    api: &ProjectsApi,
    loki_url: &str,
    loki_label: &str,
    project_id: &str,
    service_name: &str,
    state: &Mutex<ProjectState>,
) -> anyhow::Result<()> {
    let now = chrono::Utc::now();
    let start = {
        let s = state.lock().await;
        s.last_checked
    };

    // Format as RFC3339 for Loki
    let start_str = start.to_rfc3339();
    let end_str = now.to_rfc3339();

    // Query ERROR lines
    let errors = query_loki(
        loki_url,
        loki_label,
        service_name,
        "ERROR",
        &start_str,
        &end_str,
    )
    .await?;
    let warns = query_loki(
        loki_url,
        loki_label,
        service_name,
        "WARN",
        &start_str,
        &end_str,
    )
    .await?;

    // Process results
    let mut new_observations = Vec::new();

    {
        let mut s = state.lock().await;
        let now_instant = std::time::Instant::now();

        // Evict expired fingerprints
        s.seen
            .retain(|_, ts| now_instant.duration_since(*ts).as_secs() < COOLDOWN_SECS);

        for (line, labels) in &errors {
            let fp = fingerprint(line);
            if let std::collections::hash_map::Entry::Vacant(e) = s.seen.entry(fp) {
                e.insert(now_instant);
                new_observations.push(("risk", "ERROR", line.clone(), labels.clone()));
            }
        }

        for (line, labels) in &warns {
            let fp = fingerprint(line);
            if let std::collections::hash_map::Entry::Vacant(e) = s.seen.entry(fp) {
                e.insert(now_instant);
                new_observations.push(("improvement", "WARN", line.clone(), labels.clone()));
            }
        }

        s.last_checked = now;
    }

    if new_observations.is_empty() {
        return Ok(());
    }

    info!(
        "log_monitor[{service_name}]: posting {} new observation(s)",
        new_observations.len()
    );

    for (kind, severity_level, line, _labels) in &new_observations {
        let title = build_title(severity_level, line);
        let description = format!(
            "**Severity:** {severity_level}\n\n\
             **Service:** {service_name}\n\n\
             **Example log line:**\n```\n{line}\n```\n\n\
             **Assessment:** This {severity_level} was detected in the {service_name} service logs. \
             Review the log line above for context and check related code paths.",
        );

        let obs = serde_json::json!({
            "kind": kind,
            "title": title,
            "description": description,
            "severity": match *severity_level {
                "ERROR" => "medium",
                _ => "low",
            },
            "source": "log_monitor",
            "metadata": {
                "service_name": service_name,
                "fingerprint": fingerprint(line),
            }
        });

        if let Err(e) = api.post_observation(project_id, &obs).await {
            warn!("log_monitor[{service_name}]: failed to post observation: {e}");
        }
    }

    Ok(())
}

/// Query Loki for log lines matching a severity filter.
async fn query_loki(
    loki_url: &str,
    loki_label: &str,
    service_name: &str,
    severity: &str,
    start: &str,
    end: &str,
) -> anyhow::Result<Vec<(String, serde_json::Value)>> {
    // Build LogQL query: {<label>="<name>"} |= "<severity>"
    let logql = format!(r#"{{{loki_label}="{service_name}"}} |= "{severity}""#);
    let encoded_query = urlencoding::encode(&logql);
    let encoded_start = urlencoding::encode(start);
    let encoded_end = urlencoding::encode(end);

    let url = format!(
        "{loki_url}/loki/api/v1/query_range?query={encoded_query}&start={encoded_start}&end={encoded_end}&limit={QUERY_LIMIT}&direction=backward"
    );

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()?;

    let resp = client.get(&url).send().await?;
    let status = resp.status();
    let body = resp.text().await?;

    if !status.is_success() {
        let end = body.floor_char_boundary(200.min(body.len()));
        anyhow::bail!("Loki returned {status}: {}", &body[..end]);
    }

    let parsed: serde_json::Value = serde_json::from_str(&body)?;
    let mut results = Vec::new();

    if let Some(streams) = parsed["data"]["result"].as_array() {
        for stream in streams {
            let labels = stream
                .get("stream")
                .cloned()
                .unwrap_or(serde_json::Value::Object(Default::default()));

            if let Some(values) = stream["values"].as_array() {
                for pair in values {
                    if let Some(arr) = pair.as_array()
                        && let Some(line) = arr.get(1).and_then(|l| l.as_str())
                    {
                        results.push((line.to_string(), labels.clone()));
                    }
                }
            }
        }
    }

    Ok(results)
}

/// Fingerprint a log line by stripping variable tokens and hashing.
fn fingerprint(line: &str) -> String {
    let normalized = normalize_line(line);
    // Take first ~256 bytes for hashing (char-boundary safe)
    let end = normalized.floor_char_boundary(256.min(normalized.len()));
    let truncated = &normalized[..end];
    let mut hasher = Sha256::new();
    hasher.update(truncated.as_bytes());
    format!("{:x}", hasher.finalize())
}

/// Strip timestamps, UUIDs, IPs, and epoch values from a log line.
fn normalize_line(line: &str) -> String {
    let mut result = line.to_string();

    // Strip ISO8601 timestamps (2026-03-01T12:34:56.789Z, etc.)
    result = strip_pattern(
        &result,
        r"\d{4}-\d{2}-\d{2}[T ]\d{2}:\d{2}:\d{2}(\.\d+)?(Z|[+-]\d{2}:?\d{2})?",
    );

    // Strip UUIDs (8-4-4-4-12 hex)
    result = strip_pattern(
        &result,
        r"[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}",
    );

    // Strip IPv4 addresses
    result = strip_pattern(&result, r"\d{1,3}\.\d{1,3}\.\d{1,3}\.\d{1,3}");

    // Strip epoch timestamps (10+ digit numbers)
    result = strip_pattern(&result, r"\b\d{10,}\b");

    // Collapse whitespace
    result.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Replace all matches of a regex pattern with empty string.
fn strip_pattern(input: &str, pattern: &str) -> String {
    // Simple regex-free approach for common patterns wouldn't scale,
    // so we use a basic manual approach to avoid adding regex dependency.
    // For the patterns we need, we can use a lightweight approach.
    manual_strip(input, pattern)
}

/// Since we want to avoid adding the `regex` crate just for this,
/// implement targeted strippers for each known pattern.
fn manual_strip(input: &str, pattern_hint: &str) -> String {
    if pattern_hint.contains("UUID") || pattern_hint.contains("0-9a-fA-F]{8}") {
        strip_uuids(input)
    } else if pattern_hint.contains(r"\d{4}-\d{2}") {
        strip_iso_timestamps(input)
    } else if pattern_hint.contains(r"\d{1,3}\.\d{1,3}") {
        strip_ipv4(input)
    } else if pattern_hint.contains(r"\d{10,}") {
        strip_epoch(input)
    } else {
        input.to_string()
    }
}

fn strip_uuids(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Try to match UUID pattern: 8-4-4-4-12 hex chars with dashes
        if i + 36 <= chars.len() && is_uuid_at(&chars, i) {
            result.push_str("<UUID>");
            i += 36;
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn is_uuid_at(chars: &[char], start: usize) -> bool {
    // 8 hex, dash, 4 hex, dash, 4 hex, dash, 4 hex, dash, 12 hex = 36 chars
    if start + 36 > chars.len() {
        return false;
    }
    let s: String = chars[start..start + 36].iter().collect();
    let parts: Vec<&str> = s.split('-').collect();
    if parts.len() != 5 {
        return false;
    }
    let expected_lens = [8, 4, 4, 4, 12];
    for (part, &expected) in parts.iter().zip(&expected_lens) {
        if part.len() != expected || !part.chars().all(|c| c.is_ascii_hexdigit()) {
            return false;
        }
    }
    true
}

fn strip_iso_timestamps(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let chars: Vec<char> = input.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        // Match pattern: YYYY-MM-DDTHH:MM:SS or YYYY-MM-DD HH:MM:SS
        if i + 19 <= chars.len() && is_iso_timestamp_at(&chars, i) {
            result.push_str("<TS>");
            // Skip the base 19 chars
            i += 19;
            // Skip optional fractional seconds (.NNN...)
            if i < chars.len() && chars[i] == '.' {
                i += 1;
                while i < chars.len() && chars[i].is_ascii_digit() {
                    i += 1;
                }
            }
            // Skip optional timezone (Z or +/-HH:MM or +/-HHMM)
            if i < chars.len() && chars[i] == 'Z' {
                i += 1;
            } else if i < chars.len() && (chars[i] == '+' || chars[i] == '-') {
                i += 1;
                // Skip HH:MM or HHMM
                let mut tz_digits = 0;
                while i < chars.len()
                    && (chars[i].is_ascii_digit() || chars[i] == ':')
                    && tz_digits < 5
                {
                    i += 1;
                    tz_digits += 1;
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }
    result
}

fn is_iso_timestamp_at(chars: &[char], start: usize) -> bool {
    if start + 19 > chars.len() {
        return false;
    }
    // YYYY-MM-DD[T ]HH:MM:SS
    chars[start].is_ascii_digit()
        && chars[start + 1].is_ascii_digit()
        && chars[start + 2].is_ascii_digit()
        && chars[start + 3].is_ascii_digit()
        && chars[start + 4] == '-'
        && chars[start + 5].is_ascii_digit()
        && chars[start + 6].is_ascii_digit()
        && chars[start + 7] == '-'
        && chars[start + 8].is_ascii_digit()
        && chars[start + 9].is_ascii_digit()
        && (chars[start + 10] == 'T' || chars[start + 10] == ' ')
        && chars[start + 11].is_ascii_digit()
        && chars[start + 12].is_ascii_digit()
        && chars[start + 13] == ':'
        && chars[start + 14].is_ascii_digit()
        && chars[start + 15].is_ascii_digit()
        && chars[start + 16] == ':'
        && chars[start + 17].is_ascii_digit()
        && chars[start + 18].is_ascii_digit()
}

fn strip_ipv4(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let bytes = input.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_digit()
            && let Some(len) = match_ipv4(bytes, i)
        {
            result.push_str("<IP>");
            i += len;
            continue;
        }
        // Preserve full UTF-8 characters (non-ASCII bytes are multi-byte)
        let ch_len = utf8_char_len(bytes[i]);
        if i + ch_len <= bytes.len() {
            result.push_str(&input[i..i + ch_len]);
        }
        i += ch_len;
    }
    result
}

/// Return the byte length of a UTF-8 character from its leading byte.
fn utf8_char_len(b: u8) -> usize {
    match b {
        0..=0x7F => 1,
        0xC0..=0xDF => 2,
        0xE0..=0xEF => 3,
        0xF0..=0xFF => 4,
        _ => 1,
    }
}

fn match_ipv4(bytes: &[u8], start: usize) -> Option<usize> {
    let mut i = start;
    let mut octets = 0;

    for octet_num in 0..4 {
        if octet_num > 0 {
            if i >= bytes.len() || bytes[i] != b'.' {
                return None;
            }
            i += 1;
        }
        let digit_start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() && i - digit_start < 4 {
            i += 1;
        }
        let digit_count = i - digit_start;
        if digit_count == 0 || digit_count > 3 {
            return None;
        }
        octets += 1;
    }

    if octets == 4 { Some(i - start) } else { None }
}

fn strip_epoch(input: &str) -> String {
    let mut result = String::with_capacity(input.len());
    let words: Vec<&str> = input.split_whitespace().collect();
    for (idx, word) in words.iter().enumerate() {
        if idx > 0 {
            result.push(' ');
        }
        if word.len() >= 10 && word.chars().all(|c| c.is_ascii_digit()) {
            result.push_str("<EPOCH>");
        } else {
            result.push_str(word);
        }
    }
    result
}

/// Build a concise title from the log line.
fn build_title(severity: &str, line: &str) -> String {
    // Try to extract a meaningful error message
    let cleaned = line.trim();
    // Take the first ~80 chars of the line for the title
    let snippet = if cleaned.len() > 80 {
        let end = cleaned.floor_char_boundary(77);
        format!("{}...", &cleaned[..end])
    } else {
        cleaned.to_string()
    };
    format!("[{severity}] {snippet}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_uuids() {
        let input = "task 550e8400-e29b-41d4-a716-446655440000 failed";
        let result = strip_uuids(input);
        assert_eq!(result, "task <UUID> failed");
    }

    #[test]
    fn test_strip_iso_timestamps() {
        let input = "2026-03-01T12:34:56.789Z error occurred";
        let result = strip_iso_timestamps(input);
        assert_eq!(result, "<TS> error occurred");
    }

    #[test]
    fn test_strip_ipv4() {
        let input = "connection from 192.168.1.1 refused";
        let result = strip_ipv4(input);
        assert_eq!(result, "connection from <IP> refused");
    }

    #[test]
    fn test_strip_epoch() {
        let input = "at 1709312096000 something happened";
        let result = strip_epoch(input);
        assert_eq!(result, "at <EPOCH> something happened");
    }

    #[test]
    fn test_fingerprint_stability() {
        let line1 = "2026-03-01T12:00:00Z task 550e8400-e29b-41d4-a716-446655440000 error: connection refused from 10.0.0.1";
        let line2 = "2026-03-01T13:00:00Z task 660e8400-e29b-41d4-a716-446655440001 error: connection refused from 10.0.0.2";
        assert_eq!(fingerprint(line1), fingerprint(line2));
    }

    #[test]
    fn test_fingerprint_different() {
        let line1 = "error: disk full";
        let line2 = "error: out of memory";
        assert_ne!(fingerprint(line1), fingerprint(line2));
    }

    #[test]
    fn test_build_title_short() {
        let title = build_title("ERROR", "something broke");
        assert_eq!(title, "[ERROR] something broke");
    }

    #[test]
    fn test_build_title_long() {
        let long_line = "a".repeat(100);
        let title = build_title("WARN", &long_line);
        assert!(title.len() < 90);
        assert!(title.ends_with("..."));
    }

    #[test]
    fn test_build_title_non_ascii() {
        // Multi-byte chars: each ä is 2 bytes, 50 of them = 100 bytes > 80
        let line = "ä".repeat(50);
        let title = build_title("ERROR", &line);
        assert!(title.ends_with("..."));
        // Must be valid UTF-8 (no panic)
        assert!(title.starts_with("[ERROR] "));
    }

    #[test]
    fn test_fingerprint_non_ascii() {
        // Ensure fingerprinting doesn't panic on non-ASCII
        let line = "Ошибка: что-то пошло не так ".repeat(20);
        let fp = fingerprint(&line);
        assert!(!fp.is_empty());
    }

    #[test]
    fn test_strip_ipv4_non_ascii() {
        let input = "Verbindung von 192.168.1.1 verweigert — Ääh";
        let result = strip_ipv4(input);
        assert_eq!(result, "Verbindung von <IP> verweigert — Ääh");
    }
}
