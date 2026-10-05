//! Task retry limits independent of agent instructions.
use crate::engine::task_source::TaskSource;
use tracing::warn;
pub async fn count_blocker_cycles(api: &dyn TaskSource, task_id: &str) -> u32 {
    match api.get_task_updates(task_id).await {
        Ok(updates) => updates
            .iter()
            .filter(|u| u["kind"].as_str() == Some("blocker"))
            .filter(|u| u["agent_id"].as_str().is_some())
            .count() as u32,
        Err(e) => {
            warn!("loop-detect: failed to fetch updates for {task_id}: {e}");
            0
        }
    }
}

/// Resolve the effective max_implement_cycles for a task's project.
pub async fn resolve_max_implement_cycles(
    api: &dyn TaskSource,
    project_id: &str,
    global_max: u32,
) -> u32 {
    match api.get_project(project_id).await {
        Ok(project) => project["metadata"]["max_implement_cycles"]
            .as_u64()
            .map(|v| v as u32)
            .unwrap_or(global_max),
        Err(e) => {
            warn!(
                "resolve_max_implement_cycles: failed to fetch project {project_id}: {e}, using global default"
            );
            global_max
        }
    }
}
