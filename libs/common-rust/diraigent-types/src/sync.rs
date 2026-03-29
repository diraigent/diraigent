//! Types for the orchestra → API sync protocol.
//!
//! Orchestra pushes task state summaries and updates to the API so the
//! web/TUI can display them. The API is a read model for these fields;
//! orchestra is the source of truth.

/// Summary of a task's current state, pushed from orchestra to API.
#[derive(Debug, Clone)]
pub struct TaskStateSummary {
    pub task_id: String,
    pub state: String,
    pub playbook_step: Option<i32>,
    pub assigned_agent_id: Option<String>,
    pub claimed_at: Option<String>,
    pub completed_at: Option<String>,
    pub state_entered_at: Option<String>,
    pub input_tokens: i64,
    pub output_tokens: i64,
    pub cost_usd: f64,
}

/// A task update (progress, blocker, artifact, etc.) pushed from orchestra.
#[derive(Debug, Clone)]
pub struct TaskUpdateEntry {
    pub id: String,
    pub task_id: String,
    pub agent_id: Option<String>,
    pub kind: String,
    pub content: String,
    pub created_at: String,
}

/// A changed file record pushed from orchestra.
#[derive(Debug, Clone)]
pub struct ChangedFileEntry {
    pub task_id: String,
    pub path: String,
    pub change_type: String,
}

/// Batch sync payload: orchestra sends this to `POST /v1/orchestra/sync`.
#[derive(Debug, Clone)]
pub struct SyncBatch {
    pub orchestra_id: String,
    pub task_states: Vec<TaskStateSummary>,
    pub task_updates: Vec<TaskUpdateEntry>,
    pub changed_files: Vec<ChangedFileEntry>,
}
