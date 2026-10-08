//! Redact read-only response projections without modifying stored project data.
use axum::{
    body::Body,
    extract::Request,
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
};
use base64::{Engine, engine::general_purpose::STANDARD};
use regex::Regex;
use serde_json::Value;
use std::sync::{
    Arc, LazyLock,
    atomic::{AtomicBool, Ordering},
};

const REDACTED: &str = "[redacted]";
const MAX_PROJECTION_BYTES: usize = 16 * 1024 * 1024;

/// Shared between the response middleware and authenticated route extractors.
#[derive(Clone, Default)]
pub struct Projection(Arc<AtomicBool>);
impl Projection {
    pub fn mark(&self) {
        self.0.store(true, Ordering::Relaxed);
    }
}

struct Policy {
    patterns: Vec<Regex>,
    private_terms: Vec<Regex>,
}

impl Policy {
    fn new(terms: impl IntoIterator<Item = String>) -> Self {
        Self {
            patterns: [
                r"(?s)-----BEGIN [A-Z ]*PRIVATE KEY-----.*?-----END [A-Z ]*PRIVATE KEY-----",
                r"\b(?:gh[pousr]_[A-Za-z0-9]{20,}|github_pat_[A-Za-z0-9_]{20,}|sk-[A-Za-z0-9_-]{20,}|dak_[A-Za-z0-9_-]{20,}|AKIA[A-Z0-9]{16}|AIza[A-Za-z0-9_-]{35}|xox[baprs]-[A-Za-z0-9-]+)\b",
                r"\beyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b",
                r#"(?i)(?:postgres(?:ql)?|mysql|redis|mongodb(?:\+srv)?|https?)://[^/\s:@]+:[^@\s/]+@[^\s\"'<>]+"#,
                r"(?i)\bBearer\s+[A-Za-z0-9._~+/=-]+",
                r#"(?i)(?:password|passwd|api[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|secret[_-]?key|secret|token|authorization)\s*[\"']?\s*[:=]\s*[\"']?[^\s\"',;<>}]+"#,
                r#"(?:/Users/|/home/|/mnt/user/)[^\s\"'<>]+"#,
                r"\b(?:10(?:\.\d{1,3}){3}|192\.168(?:\.\d{1,3}){2}|172\.(?:1[6-9]|2\d|3[01])(?:\.\d{1,3}){2}|127(?:\.\d{1,3}){3}|169\.254(?:\.\d{1,3}){2})\b",
                r"(?i)\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b",
            ].into_iter().map(|p| Regex::new(p).expect("static redaction pattern")).collect(),
            private_terms: terms.into_iter().filter(|s| !s.trim().is_empty())
                .map(|s| Regex::new(&format!("(?i){}",regex::escape(s.trim()))).expect("escaped term")).collect(),
        }
    }

    fn text(&self, value: &str) -> String {
        self.patterns
            .iter()
            .chain(&self.private_terms)
            .fold(value.to_owned(), |text, pattern| {
                pattern.replace_all(&text, REDACTED).into_owned()
            })
    }

    fn json(&self, value: &mut Value) {
        match value {
            Value::String(s) => *s = self.text(s),
            Value::Array(items) => {
                for item in items {
                    self.json(item);
                }
            }
            Value::Object(fields) => {
                // Binary source files cannot be reviewed safely as arbitrary bytes.
                if fields.get("encoding").and_then(Value::as_str) == Some("base64")
                    && let Some(Value::String(content)) = fields.get_mut("content")
                {
                    let clean = STANDARD
                        .decode(content.as_bytes())
                        .ok()
                        .and_then(|bytes| String::from_utf8(bytes).ok())
                        .map(|text| self.text(&text))
                        .unwrap_or_else(|| "Binary content redacted from read-only view".into());
                    *content = STANDARD.encode(clean.as_bytes());
                    fields.insert("size".into(), Value::from(clean.len()));
                }
                for (key, mut item) in std::mem::take(fields) {
                    let normalized: String = key
                        .to_ascii_lowercase()
                        .chars()
                        .filter(|c| c.is_ascii_alphanumeric())
                        .collect();
                    if matches!(
                        normalized.as_str(),
                        "password"
                            | "passwd"
                            | "apikey"
                            | "accesstoken"
                            | "refreshtoken"
                            | "idtoken"
                            | "clientsecret"
                            | "authorization"
                            | "privatekey"
                            | "secret"
                            | "secretkey"
                            | "awssecretaccesskey"
                            | "credentials"
                            | "wrappeddek"
                            | "connectionstring"
                            | "databaseurl"
                            | "databasepassword"
                            | "token"
                            | "gitroot"
                            | "reporoot"
                            | "repopath"
                            | "projectspath"
                    ) && !item.is_null()
                    {
                        item = Value::String(REDACTED.into());
                    } else {
                        self.json(&mut item);
                    }
                    fields.insert(self.text(&key), item);
                }
            }
            _ => {}
        }
    }
}

static POLICY: LazyLock<Policy> = LazyLock::new(|| {
    Policy::new(
        std::env::var("READ_ONLY_REDACT_TERMS")
            .unwrap_or_default()
            .split(',')
            .map(str::to_owned),
    )
});

pub async fn redact_response(mut request: Request, next: Next) -> Response {
    let head = request.method() == axum::http::Method::HEAD;
    let projection = Projection::default();
    request.extensions_mut().insert(projection.clone());
    let response = next.run(request).await;
    if !projection.0.load(Ordering::Relaxed) {
        return response;
    }
    let (mut parts, body) = response.into_parts();
    if head || parts.status == axum::http::StatusCode::NO_CONTENT {
        parts.headers.remove(header::ETAG);
        parts
            .headers
            .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
        return Response::from_parts(parts, Body::empty());
    }
    let json = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("application/json"));
    let text = parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.starts_with("text/"));
    if !json && !text {
        // An authenticated viewer must not receive an unhandled binary/stream body.
        return (
            axum::http::StatusCode::FORBIDDEN,
            "Content unavailable in read-only projection",
        )
            .into_response();
    }
    let Ok(bytes) = axum::body::to_bytes(body, MAX_PROJECTION_BYTES).await else {
        return (
            axum::http::StatusCode::PAYLOAD_TOO_LARGE,
            "Read-only projection too large",
        )
            .into_response();
    };
    let clean = if json {
        let Ok(mut value) = serde_json::from_slice::<Value>(&bytes) else {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Invalid read-only projection",
            )
                .into_response();
        };
        POLICY.json(&mut value);
        serde_json::to_vec(&value).expect("JSON value serialization")
    } else {
        POLICY.text(&String::from_utf8_lossy(&bytes)).into_bytes()
    };
    parts.headers.remove(header::CONTENT_LENGTH);
    parts.headers.remove(header::ETAG);
    parts
        .headers
        .insert(header::CACHE_CONTROL, "private, no-store".parse().unwrap());
    Response::from_parts(parts, Body::from(clean))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_snapshots_diffs_and_encoded_source_are_redacted() {
        let policy = Policy::new(["git.private.example".to_owned()]);
        let source = "password=private-value\nvisit https://git.private.example/project";
        let mut value = serde_json::json!({"context":{"api_key":"private-key","file":"src/app.rs"},"diff":"+url=http://192.168.5.7:3000\n+root=/Users/test/private/repo","before":{"note":"email user@example.test"},"source":{"encoding":"base64","content":STANDARD.encode(source)}});
        policy.json(&mut value);
        let text = value.to_string();
        for private in [
            "private-key",
            "192.168.5.7",
            "/Users/test",
            "user@example.test",
        ] {
            assert!(!text.contains(private));
        }
        assert_eq!(value["context"]["file"], "src/app.rs");
        let decoded = String::from_utf8(
            STANDARD
                .decode(value["source"]["content"].as_str().unwrap())
                .unwrap(),
        )
        .unwrap();
        assert!(!decoded.contains("private-value"));
        assert!(!decoded.contains("git.private.example"));
        assert!(decoded.contains("[redacted]"));
    }
}
