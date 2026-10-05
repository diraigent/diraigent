pub use diraigent_types::TaskProfile;

/// Override retry checks for a task; otherwise derive from its execution mode.
pub fn is_retriable(options: Option<&serde_json::Value>, mode: &str) -> bool {
    options
        .and_then(|o| o["retriable"].as_bool())
        .unwrap_or_else(|| TaskProfile::for_mode(mode).is_execution())
}
