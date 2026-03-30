//! Task state machine rules shared between the API and orchestra.
//!
//! The API uses these for validation on human-initiated transitions.
//! The orchestra uses these as the authoritative state machine when
//! running in local orchestration mode.

/// Lifecycle states have fixed transition rules. Everything else is
/// a playbook step name (active execution state).
pub fn is_lifecycle_state(s: &str) -> bool {
    matches!(s, "backlog" | "ready" | "done" | "cancelled") || s.starts_with("wait:")
}

/// Returns true if the state is a `wait:<step>` inter-step state.
pub fn is_wait_state(s: &str) -> bool {
    s.starts_with("wait:")
}

/// Extract the next step name from a `wait:<step>` state.
pub fn wait_target(s: &str) -> Option<&str> {
    s.strip_prefix("wait:")
}

/// Validate whether a state transition is allowed.
///
/// ```text
///   backlog    → ready, cancelled
///   ready      → <step_name>, backlog, cancelled
///   <step>     → done, ready, cancelled, wait:<next>
///   wait:<s>   → <s> (via claim), cancelled
///   done       → backlog, human_review
///   cancelled  → backlog
/// ```
pub fn can_transition(current: &str, target: &str) -> bool {
    match current {
        "backlog" => matches!(target, "ready" | "cancelled"),
        "ready" => {
            // ready → any step name, or back to backlog/cancelled
            !is_lifecycle_state(target) || matches!(target, "backlog" | "cancelled")
        }
        "done" => {
            // done is terminal — reopen to backlog, or move to human_review
            target == "backlog" || target == "human_review"
        }
        "cancelled" => target == "backlog",
        _ if is_wait_state(current) => {
            // wait:<next> → the named step (via claim) or cancelled
            let next = wait_target(current).unwrap_or("");
            target == next || target == "cancelled"
        }
        _ => {
            // Current state is a step name (e.g. implement, review, human_review)
            // Can go to done (final), wait:<next> (pipeline), ready (release), or cancelled
            matches!(target, "done" | "ready" | "cancelled") || is_wait_state(target)
        }
    }
}

/// Check if a playbook step is retriable (can be regressed to on rejection).
///
/// Reads `"retriable"` from the step JSON if present, otherwise falls back
/// to name-prefix classification (implement-like steps are retriable).
pub fn is_retriable_step(step: &serde_json::Value) -> bool {
    use crate::StepProfile;
    if let Some(v) = step.get("retriable").and_then(|v| v.as_bool()) {
        return v;
    }
    let name = step["name"].as_str().unwrap_or("");
    StepProfile::for_step(name).is_implement()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lifecycle_states() {
        assert!(is_lifecycle_state("backlog"));
        assert!(is_lifecycle_state("ready"));
        assert!(is_lifecycle_state("done"));
        assert!(is_lifecycle_state("cancelled"));
        assert!(is_lifecycle_state("wait:review"));
        assert!(!is_lifecycle_state("implement"));
        assert!(!is_lifecycle_state("review"));
    }

    #[test]
    fn wait_states() {
        assert!(is_wait_state("wait:review"));
        assert!(!is_wait_state("ready"));
        assert_eq!(wait_target("wait:review"), Some("review"));
        assert_eq!(wait_target("ready"), None);
    }

    #[test]
    fn transitions() {
        // backlog
        assert!(can_transition("backlog", "ready"));
        assert!(can_transition("backlog", "cancelled"));
        assert!(!can_transition("backlog", "done"));

        // ready
        assert!(can_transition("ready", "implement"));
        assert!(can_transition("ready", "backlog"));
        assert!(!can_transition("ready", "done"));

        // step
        assert!(can_transition("implement", "done"));
        assert!(can_transition("implement", "ready"));
        assert!(can_transition("implement", "cancelled"));
        assert!(can_transition("implement", "wait:review"));
        assert!(!can_transition("implement", "backlog"));

        // wait
        assert!(can_transition("wait:review", "review"));
        assert!(can_transition("wait:review", "cancelled"));
        assert!(!can_transition("wait:review", "implement"));

        // done
        assert!(can_transition("done", "backlog"));
        assert!(can_transition("done", "human_review"));
        assert!(!can_transition("done", "ready"));

        // cancelled
        assert!(can_transition("cancelled", "backlog"));
        assert!(!can_transition("cancelled", "ready"));
    }
}
