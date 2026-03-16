use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Paginated list response for workflow runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRunList {
    pub workflow_runs: Vec<WorkflowRun>,
    pub total_count: i64,
}

/// A single workflow run (CI pipeline execution).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub head_branch: String,
    #[serde(default)]
    pub head_sha: String,
    #[serde(default)]
    pub event: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    #[serde(default)]
    pub workflow_id: i64,
    #[serde(default)]
    pub run_number: i64,
    #[serde(default)]
    pub html_url: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
    pub updated_at: Option<DateTime<Utc>>,
    pub run_started_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub triggering_actor: Option<Actor>,
}

/// Actor (user) who triggered the run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Actor {
    pub id: i64,
    #[serde(default)]
    pub login: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
}

/// Paginated list response for workflow jobs.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowJobList {
    pub total_count: i64,
    pub jobs: Vec<WorkflowJob>,
}

/// A single job within a workflow run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowJob {
    pub id: i64,
    pub run_id: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub runner_name: Option<String>,
    #[serde(default)]
    pub steps: Vec<WorkflowStep>,
}

/// A single step within a job.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkflowStep {
    pub number: i64,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub conclusion: Option<String>,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
}
