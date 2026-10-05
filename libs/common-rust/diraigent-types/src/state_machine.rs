//! Task lifecycle shared by the API and local workers.
pub fn is_valid_state(state: &str) -> bool {
    matches!(
        state,
        "backlog" | "ready" | "working" | "done" | "cancelled" | "human_review"
    )
}
pub fn can_transition(current: &str, target: &str) -> bool {
    match current {
        "backlog" => matches!(target, "ready" | "cancelled"),
        "ready" => matches!(target, "working" | "backlog" | "cancelled" | "human_review"),
        "working" => matches!(target, "done" | "ready" | "cancelled" | "human_review"),
        "human_review" => matches!(target, "done" | "ready" | "backlog" | "cancelled"),
        "done" => matches!(target, "backlog" | "ready" | "human_review"),
        "cancelled" => target == "backlog",
        _ => false,
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn direct_execution_and_review() {
        for (from, to) in [
            ("backlog", "ready"),
            ("ready", "working"),
            ("working", "done"),
            ("working", "human_review"),
            ("human_review", "ready"),
            ("human_review", "done"),
        ] {
            assert!(can_transition(from, to));
        }
        assert!(!can_transition("ready", "done"));
        assert!(!can_transition("backlog", "working"));
    }
    #[test]
    fn arbitrary_stages_are_rejected() {
        for state in ["implement", "review", "wait:review", "custom", ""] {
            assert!(!can_transition("ready", state));
            assert!(!can_transition(state, "done"));
        }
    }
}
