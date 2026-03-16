// ── Task lifecycle state values ──

pub const STATE_READY: &str = "ready";
pub const STATE_DONE: &str = "done";
pub const STATE_CANCELLED: &str = "cancelled";
pub const STATE_HUMAN_REVIEW: &str = "human_review";
pub const STATE_BACKLOG: &str = "backlog";

/// Parsed task state — avoids string comparisons scattered across the codebase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskState {
    Ready,
    Done,
    Cancelled,
    HumanReview,
    Backlog,
    /// A `wait:<step>` pause state.
    Wait(String),
    /// An active playbook step (e.g. "implement", "review").
    Step(String),
}

impl TaskState {
    /// Parse a state string from the API into a typed enum.
    pub fn parse(s: &str) -> Self {
        match s {
            STATE_READY => Self::Ready,
            STATE_DONE => Self::Done,
            STATE_CANCELLED => Self::Cancelled,
            STATE_HUMAN_REVIEW => Self::HumanReview,
            STATE_BACKLOG => Self::Backlog,
            other if other.starts_with("wait:") => {
                Self::Wait(other.strip_prefix("wait:").unwrap_or("unknown").to_string())
            }
            "" => Self::Step(String::new()), // empty/missing
            other => Self::Step(other.to_string()),
        }
    }
}
