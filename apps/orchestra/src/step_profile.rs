// Re-export StepProfile from the shared diraigent-types crate.
// This keeps existing `use crate::step_profile::StepProfile` imports working
// throughout the orchestra codebase while the canonical definition lives in
// diraigent-types (shared with the API).
pub use diraigent_types::StepProfile;

/// Check if a playbook step is retriable (can be regressed to on rejection).
///
/// Reads `"retriable"` from the step JSON if present, otherwise falls back
/// to name-prefix classification (implement-like steps are retriable).
pub fn is_retriable(step: &serde_json::Value) -> bool {
    if let Some(v) = step.get("retriable").and_then(|v| v.as_bool()) {
        return v;
    }
    // Fallback: classify by name prefix (backward compat for steps without the field)
    let name = step["name"].as_str().unwrap_or("");
    StepProfile::for_step(name) == StepProfile::Implement
}
