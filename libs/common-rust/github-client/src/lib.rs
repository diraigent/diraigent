//! GitHub Actions REST API client.
//!
//! Supports fetching workflow runs, jobs, and steps from GitHub.

pub mod models;

pub use models::*;

use std::time::Duration;

/// Errors returned by the GitHub API client.
#[derive(Debug, thiserror::Error)]
pub enum GitHubError {
    /// The request failed at the HTTP transport level.
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    /// The server returned 401 Unauthorized.
    #[error("authentication failed (401): check your access token")]
    Unauthorized,

    /// The server returned 403 Forbidden.
    #[error("forbidden (403): insufficient permissions")]
    Forbidden,

    /// The requested resource was not found (404).
    #[error("not found (404): {url}")]
    NotFound { url: String },

    /// The server returned 403 with rate-limit headers.
    #[error("rate limited (403): retry after rate limit resets")]
    RateLimited,

    /// The server returned an unexpected HTTP error.
    #[error("HTTP {status}: {body}")]
    HttpError { status: u16, body: String },

    /// Failed to deserialize the response body.
    #[error("failed to parse response: {0}")]
    Deserialize(String),
}

/// Result type for GitHub client operations.
pub type Result<T> = std::result::Result<T, GitHubError>;

/// HTTP client for the GitHub Actions REST API.
///
/// # Example
///
/// ```no_run
/// # async fn example() -> github_client::Result<()> {
/// let client = github_client::GitHubClient::new(
///     "https://api.github.com",
///     Some("ghp_your-token".to_string()),
/// );
/// let runs = client.list_runs("owner", "repo", 1, 10).await?;
/// println!("Total runs: {}", runs.total_count);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct GitHubClient {
    http: reqwest::Client,
    base_url: String,
    token: Option<String>,
}

impl GitHubClient {
    /// Create a new GitHub API client.
    ///
    /// - `base_url`: The GitHub API URL (e.g. `https://api.github.com`).
    ///   Trailing slashes are stripped.
    /// - `token`: Optional personal access token for Bearer authentication.
    pub fn new(base_url: impl Into<String>, token: Option<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("diraigent-github-client")
            .build()
            .expect("failed to build reqwest client");

        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token,
        }
    }

    /// Create a client with a custom `reqwest::Client` (useful for testing).
    pub fn with_http_client(
        http: reqwest::Client,
        base_url: impl Into<String>,
        token: Option<String>,
    ) -> Self {
        Self {
            http,
            base_url: base_url.into().trim_end_matches('/').to_string(),
            token,
        }
    }

    /// List workflow runs for a repository.
    ///
    /// Corresponds to `GET /repos/{owner}/{repo}/actions/runs`.
    pub async fn list_runs(
        &self,
        owner: &str,
        repo: &str,
        page: u32,
        per_page: u32,
    ) -> Result<WorkflowRunList> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs?page={}&per_page={}",
            self.base_url, owner, repo, page, per_page
        );
        self.get_json(&url).await
    }

    /// Get a single workflow run by ID.
    ///
    /// Corresponds to `GET /repos/{owner}/{repo}/actions/runs/{run_id}`.
    pub async fn get_run(&self, owner: &str, repo: &str, run_id: i64) -> Result<WorkflowRun> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}",
            self.base_url, owner, repo, run_id
        );
        self.get_json(&url).await
    }

    /// List jobs for a workflow run.
    ///
    /// Corresponds to `GET /repos/{owner}/{repo}/actions/runs/{run_id}/jobs`.
    pub async fn list_jobs(
        &self,
        owner: &str,
        repo: &str,
        run_id: i64,
        page: u32,
        per_page: u32,
    ) -> Result<WorkflowJobList> {
        let url = format!(
            "{}/repos/{}/{}/actions/runs/{}/jobs?page={}&per_page={}",
            self.base_url, owner, repo, run_id, page, per_page
        );
        self.get_json(&url).await
    }

    /// Get a single job by ID (includes steps).
    ///
    /// Corresponds to `GET /repos/{owner}/{repo}/actions/jobs/{job_id}`.
    pub async fn get_job(&self, owner: &str, repo: &str, job_id: i64) -> Result<WorkflowJob> {
        let url = format!(
            "{}/repos/{}/{}/actions/jobs/{}",
            self.base_url, owner, repo, job_id
        );
        self.get_json(&url).await
    }

    /// Send an authenticated GET request and deserialize the JSON response.
    async fn get_json<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T> {
        let mut req = self
            .http
            .get(url)
            .header("Accept", "application/vnd.github+json")
            .header("X-GitHub-Api-Version", "2022-11-28");

        if let Some(ref token) = self.token {
            req = req.bearer_auth(token);
        }

        tracing::debug!(url = %url, "GitHub API request");

        let response = req.send().await?;
        let status = response.status();

        if !status.is_success() {
            return Err(match status.as_u16() {
                401 => GitHubError::Unauthorized,
                403 => {
                    // Check for rate limiting via x-ratelimit-remaining header
                    let is_rate_limited = response
                        .headers()
                        .get("x-ratelimit-remaining")
                        .and_then(|v| v.to_str().ok())
                        .and_then(|v| v.parse::<u64>().ok())
                        .is_some_and(|remaining| remaining == 0);

                    if is_rate_limited {
                        GitHubError::RateLimited
                    } else {
                        GitHubError::Forbidden
                    }
                }
                404 => GitHubError::NotFound {
                    url: url.to_string(),
                },
                _ => {
                    let body = response
                        .text()
                        .await
                        .unwrap_or_else(|_| "<unreadable body>".to_string());
                    GitHubError::HttpError {
                        status: status.as_u16(),
                        body,
                    }
                }
            });
        }

        let bytes = response.bytes().await?;
        serde_json::from_slice(&bytes).map_err(|e| {
            tracing::warn!(
                url = %url,
                error = %e,
                body = %String::from_utf8_lossy(&bytes),
                "Failed to deserialize GitHub API response"
            );
            GitHubError::Deserialize(e.to_string())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use wiremock::matchers::{header, method, path, query_param};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn test_run() -> serde_json::Value {
        serde_json::json!({
            "id": 42,
            "name": "CI",
            "head_branch": "main",
            "head_sha": "abc123def456",
            "event": "push",
            "status": "completed",
            "conclusion": "success",
            "workflow_id": 123,
            "run_number": 7,
            "html_url": "https://github.com/owner/repo/actions/runs/42",
            "created_at": "2026-03-15T10:00:00Z",
            "updated_at": "2026-03-15T10:05:00Z",
            "run_started_at": "2026-03-15T10:00:01Z",
            "triggering_actor": {
                "id": 1,
                "login": "admin",
                "avatar_url": "https://avatars.githubusercontent.com/u/1"
            }
        })
    }

    fn test_job() -> serde_json::Value {
        serde_json::json!({
            "id": 100,
            "run_id": 42,
            "name": "build",
            "status": "completed",
            "conclusion": "success",
            "started_at": "2026-03-15T10:00:02Z",
            "completed_at": "2026-03-15T10:04:30Z",
            "runner_name": "ubuntu-latest",
            "steps": [
                {
                    "number": 1,
                    "name": "Checkout",
                    "status": "completed",
                    "conclusion": "success",
                    "started_at": "2026-03-15T10:00:03Z",
                    "completed_at": "2026-03-15T10:00:10Z"
                },
                {
                    "number": 2,
                    "name": "Build",
                    "status": "completed",
                    "conclusion": "success",
                    "started_at": "2026-03-15T10:00:10Z",
                    "completed_at": "2026-03-15T10:04:00Z"
                }
            ]
        })
    }

    fn client_for(server: &MockServer, token: Option<String>) -> GitHubClient {
        GitHubClient::new(server.uri(), token)
    }

    // ── list_runs ───────────────────────────────────────────

    #[tokio::test]
    async fn list_runs_returns_paginated_results() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .and(query_param("page", "1"))
            .and(query_param("per_page", "10"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "workflow_runs": [test_run()],
                "total_count": 1
            })))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let result = client.list_runs("owner", "repo", 1, 10).await.unwrap();

        assert_eq!(result.total_count, 1);
        assert_eq!(result.workflow_runs.len(), 1);
        assert_eq!(result.workflow_runs[0].id, 42);
        assert_eq!(result.workflow_runs[0].name, "CI");
        assert_eq!(result.workflow_runs[0].head_branch, "main");
        assert_eq!(result.workflow_runs[0].status, "completed");
    }

    #[tokio::test]
    async fn list_runs_sends_bearer_token() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .and(header("Authorization", "Bearer my-secret-token"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "workflow_runs": [],
                "total_count": 0
            })))
            .mount(&server)
            .await;

        let client = client_for(&server, Some("my-secret-token".to_string()));
        let result = client.list_runs("owner", "repo", 1, 20).await.unwrap();

        assert_eq!(result.total_count, 0);
        assert!(result.workflow_runs.is_empty());
    }

    #[tokio::test]
    async fn sends_github_specific_headers() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .and(header("Accept", "application/vnd.github+json"))
            .and(header("X-GitHub-Api-Version", "2022-11-28"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "workflow_runs": [],
                "total_count": 0
            })))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let result = client.list_runs("owner", "repo", 1, 10).await.unwrap();

        assert_eq!(result.total_count, 0);
    }

    // ── get_run ─────────────────────────────────────────────

    #[tokio::test]
    async fn get_run_returns_single_run() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs/42"))
            .respond_with(ResponseTemplate::new(200).set_body_json(test_run()))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let run = client.get_run("owner", "repo", 42).await.unwrap();

        assert_eq!(run.id, 42);
        assert_eq!(run.name, "CI");
        assert_eq!(run.head_sha, "abc123def456");
        assert_eq!(run.event, "push");
        assert_eq!(run.run_number, 7);
        assert!(run.created_at.is_some());
        assert!(run.triggering_actor.is_some());
        assert_eq!(run.triggering_actor.unwrap().login, "admin");
    }

    // ── list_jobs ───────────────────────────────────────────

    #[tokio::test]
    async fn list_jobs_returns_jobs_for_run() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs/42/jobs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "jobs": [test_job()],
                "total_count": 1
            })))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let result = client.list_jobs("owner", "repo", 42, 1, 30).await.unwrap();

        assert_eq!(result.total_count, 1);
        assert_eq!(result.jobs.len(), 1);
        assert_eq!(result.jobs[0].id, 100);
        assert_eq!(result.jobs[0].run_id, 42);
        assert_eq!(result.jobs[0].name, "build");
        assert_eq!(result.jobs[0].conclusion, Some("success".to_string()));
    }

    // ── get_job ─────────────────────────────────────────────

    #[tokio::test]
    async fn get_job_returns_job_with_steps() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/jobs/100"))
            .respond_with(ResponseTemplate::new(200).set_body_json(test_job()))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let job = client.get_job("owner", "repo", 100).await.unwrap();

        assert_eq!(job.id, 100);
        assert_eq!(job.run_id, 42);
        assert_eq!(job.steps.len(), 2);
        assert_eq!(job.steps[0].name, "Checkout");
        assert_eq!(job.steps[0].number, 1);
        assert_eq!(job.steps[1].name, "Build");
        assert_eq!(job.steps[1].number, 2);
    }

    // ── Error handling ──────────────────────────────────────

    #[tokio::test]
    async fn returns_unauthorized_on_401() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .respond_with(ResponseTemplate::new(401))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let err = client.list_runs("owner", "repo", 1, 10).await.unwrap_err();

        assert!(matches!(err, GitHubError::Unauthorized));
        assert!(err.to_string().contains("401"));
    }

    #[tokio::test]
    async fn returns_not_found_on_404() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs/999"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let err = client.get_run("owner", "repo", 999).await.unwrap_err();

        match err {
            GitHubError::NotFound { url } => {
                assert!(url.contains("/actions/runs/999"));
            }
            other => panic!("expected NotFound, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn returns_forbidden_on_403() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs/1"))
            .respond_with(ResponseTemplate::new(403))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let err = client.get_run("owner", "repo", 1).await.unwrap_err();

        assert!(matches!(err, GitHubError::Forbidden));
    }

    #[tokio::test]
    async fn returns_rate_limited_on_403_with_zero_remaining() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .respond_with(
                ResponseTemplate::new(403)
                    .insert_header("x-ratelimit-remaining", "0")
                    .insert_header("x-ratelimit-limit", "60"),
            )
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let err = client.list_runs("owner", "repo", 1, 10).await.unwrap_err();

        assert!(matches!(err, GitHubError::RateLimited));
    }

    #[tokio::test]
    async fn returns_http_error_on_500() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .respond_with(ResponseTemplate::new(500).set_body_string("Internal Server Error"))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let err = client.list_runs("owner", "repo", 1, 10).await.unwrap_err();

        match err {
            GitHubError::HttpError { status, body } => {
                assert_eq!(status, 500);
                assert_eq!(body, "Internal Server Error");
            }
            other => panic!("expected HttpError, got: {other:?}"),
        }
    }

    #[tokio::test]
    async fn returns_deserialize_error_on_invalid_json() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_string("not json"))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let err = client.list_runs("owner", "repo", 1, 10).await.unwrap_err();

        assert!(matches!(err, GitHubError::Deserialize(_)));
    }

    // ── URL construction ────────────────────────────────────

    #[tokio::test]
    async fn strips_trailing_slash_from_base_url() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "workflow_runs": [],
                "total_count": 0
            })))
            .mount(&server)
            .await;

        // Pass base URL with trailing slash
        let client = GitHubClient::new(format!("{}/", server.uri()), None);
        let result = client.list_runs("owner", "repo", 1, 10).await.unwrap();

        assert_eq!(result.total_count, 0);
    }

    #[tokio::test]
    async fn no_auth_header_when_token_is_none() {
        let server = MockServer::start().await;

        // This mock requires NO Authorization header
        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "workflow_runs": [],
                "total_count": 0
            })))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let result = client.list_runs("owner", "repo", 1, 10).await.unwrap();

        assert_eq!(result.total_count, 0);
    }

    // ── Partial response deserialization ─────────────────────

    #[tokio::test]
    async fn handles_minimal_run_response() {
        let server = MockServer::start().await;

        Mock::given(method("GET"))
            .and(path("/repos/owner/repo/actions/runs/1"))
            .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
                "id": 1
            })))
            .mount(&server)
            .await;

        let client = client_for(&server, None);
        let run = client.get_run("owner", "repo", 1).await.unwrap();

        assert_eq!(run.id, 1);
        assert_eq!(run.name, ""); // default
        assert_eq!(run.status, ""); // default
        assert!(run.conclusion.is_none());
        assert!(run.triggering_actor.is_none());
    }
}
