/// Execution profile for a single task. It never advances task states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TaskProfile {
    /// Review task — read-only, no heavy context.
    Review,
    /// Delivery task — minimal context, git operations only.
    Delivery,
    /// Research task — full context for analysis, no code changes.
    Research,
    /// Execution task (default) — full context, full toolset.
    Execute,
}

impl TaskProfile {
    /// Select execution options by task mode.
    pub fn for_mode(name: &str) -> Self {
        if name.starts_with("review") {
            TaskProfile::Review
        } else if name.starts_with("merge") || name.starts_with("deliver") {
            TaskProfile::Delivery
        } else if name.starts_with("dream") || name == "research" {
            TaskProfile::Research
        } else {
            TaskProfile::Execute
        }
    }

    /// Returns true if this profile represents an execution task.
    pub fn is_execution(&self) -> bool {
        matches!(self, TaskProfile::Execute)
    }
}
