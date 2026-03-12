/// Canonical step profile enum for all playbook step types.
///
/// Both the API (repository/transitions.rs) and the orchestra (worker.rs,
/// prompt.rs) resolve step behaviour through this single source of truth,
/// so adding a new step type only requires one match arm here.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepProfile {
    /// Code-review step — read-only, no heavy context.
    Review,
    /// Merge / deliver step — minimal context, git operations only.
    Merge,
    /// Dream / ideation step — full context for analysis, no code changes.
    Dream,
    /// Implement / rework step (default) — full context, full toolset.
    Implement,
}

impl StepProfile {
    /// Classify a step by its name prefix.
    pub fn for_step(name: &str) -> Self {
        if name.starts_with("review") {
            StepProfile::Review
        } else if name.starts_with("merge") || name.starts_with("deliver") {
            StepProfile::Merge
        } else if name.starts_with("dream") {
            StepProfile::Dream
        } else {
            StepProfile::Implement
        }
    }

    /// Returns true if this profile represents an implement-type step.
    pub fn is_implement(&self) -> bool {
        matches!(self, StepProfile::Implement)
    }
}
