//! Event trigger engine: auto-creates observations when events match configured rules.

use tracing::warn;
use uuid::Uuid;

use crate::AppState;
use crate::error::AppError;
use crate::models::*;
use crate::repository;

/// Process event triggers: find matching rules and create observations.
///
/// This is best-effort — errors are logged but do not propagate to the caller.
pub async fn process_event_triggers(
    state: &AppState,
    project_id: Uuid,
    event: &Event,
) -> Result<Vec<Observation>, AppError> {
    let rules = repository::find_matching_rules(
        &state.pool,
        project_id,
        &event.kind,
        &event.source,
        Some(&event.severity),
    )
    .await?;

    let mut observations = Vec::new();

    for rule in &rules {
        let title = render_template(&rule.title_template, event);
        let description = rule
            .description_template
            .as_deref()
            .map(|t| render_template(t, event));

        let evidence = serde_json::json!({
            "event_id": event.id,
            "rule_id": rule.id,
            "event_kind": event.kind,
        });

        let req = CreateObservation {
            agent_id: None,
            kind: Some(rule.observation_kind.clone()),
            title,
            description,
            severity: Some(rule.observation_severity.clone()),
            source: Some("event_trigger".to_string()),
            source_task_id: None,
            evidence: Some(evidence),
            metadata: Some(serde_json::json!({})),
        };

        match state.db.create_observation(project_id, &req).await {
            Ok(obs) => {
                state.fire_event(
                    project_id,
                    "observation.auto_created",
                    "observation",
                    obs.id,
                    None,
                    None,
                    serde_json::json!({
                        "observation_id": obs.id,
                        "rule_id": rule.id,
                        "event_id": event.id,
                    }),
                );
                observations.push(obs);
            }
            Err(e) => {
                warn!(
                    event_id = %event.id,
                    rule_id = %rule.id,
                    error = %e,
                    "Failed to create observation from event trigger"
                );
            }
        }
    }

    Ok(observations)
}

/// Replace template placeholders with event field values.
///
/// Supported placeholders:
/// - `{{event.title}}` → event.title
/// - `{{event.kind}}` → event.kind
/// - `{{event.source}}` → event.source
/// - `{{event.severity}}` → event.severity
pub fn render_template(template: &str, event: &Event) -> String {
    template
        .replace("{{event.title}}", &event.title)
        .replace("{{event.kind}}", &event.kind)
        .replace("{{event.source}}", &event.source)
        .replace("{{event.severity}}", &event.severity)
}

/// Check whether an event severity meets or exceeds the given threshold.
///
/// Severity ordering: info(0) < warning(1) < error(2) < critical(3).
pub fn severity_gte_matches(event_severity: &str, threshold: &str) -> bool {
    let event_level = severity_level(event_severity);
    let threshold_level = severity_level(threshold);
    event_level >= threshold_level
}

fn severity_level(s: &str) -> i32 {
    match s {
        "info" => 0,
        "warning" => 1,
        "error" => 2,
        "critical" => 3,
        _ => 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_render_template_all_placeholders() {
        let event = Event {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            kind: "deploy".to_string(),
            source: "ci-pipeline".to_string(),
            title: "Deployment failed".to_string(),
            description: None,
            severity: "error".to_string(),
            metadata: serde_json::json!({}),
            related_task_id: None,
            agent_id: None,
            created_at: chrono::Utc::now(),
        };

        let template =
            "[{{event.kind}}] {{event.title}} from {{event.source}} ({{event.severity}})";
        let result = render_template(template, &event);
        assert_eq!(
            result,
            "[deploy] Deployment failed from ci-pipeline (error)"
        );
    }

    #[test]
    fn test_render_template_no_placeholders() {
        let event = Event {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            kind: "test".to_string(),
            source: "unit".to_string(),
            title: "Test".to_string(),
            description: None,
            severity: "info".to_string(),
            metadata: serde_json::json!({}),
            related_task_id: None,
            agent_id: None,
            created_at: chrono::Utc::now(),
        };

        let result = render_template("Static observation title", &event);
        assert_eq!(result, "Static observation title");
    }

    #[test]
    fn test_render_template_repeated_placeholder() {
        let event = Event {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            kind: "error".to_string(),
            source: "api".to_string(),
            title: "Crash".to_string(),
            description: None,
            severity: "critical".to_string(),
            metadata: serde_json::json!({}),
            related_task_id: None,
            agent_id: None,
            created_at: chrono::Utc::now(),
        };

        let result = render_template("{{event.kind}} - {{event.kind}}", &event);
        assert_eq!(result, "error - error");
    }

    #[test]
    fn test_severity_gte_matches_exact() {
        assert!(severity_gte_matches("warning", "warning"));
    }

    #[test]
    fn test_severity_gte_matches_higher() {
        assert!(severity_gte_matches("error", "warning"));
        assert!(severity_gte_matches("critical", "info"));
        assert!(severity_gte_matches("critical", "error"));
    }

    #[test]
    fn test_severity_gte_matches_lower() {
        assert!(!severity_gte_matches("info", "warning"));
        assert!(!severity_gte_matches("warning", "error"));
        assert!(!severity_gte_matches("error", "critical"));
    }

    #[test]
    fn test_severity_gte_matches_info_threshold() {
        // Everything matches info threshold
        assert!(severity_gte_matches("info", "info"));
        assert!(severity_gte_matches("warning", "info"));
        assert!(severity_gte_matches("error", "info"));
        assert!(severity_gte_matches("critical", "info"));
    }

    #[test]
    fn test_severity_gte_matches_unknown_defaults_to_info() {
        assert!(severity_gte_matches("unknown", "info"));
        assert!(!severity_gte_matches("unknown", "warning"));
    }
}
