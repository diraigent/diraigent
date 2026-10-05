//! Explicit publication policy and allowlisted projections for anonymous viewing.
//! This module does not grant member permissions or access encryption keys.

use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    error::AppError,
    models::{Decision, Knowledge, Project, Task, Tenant, Work},
};

pub const SPECTATOR_ENABLED: &str = "spectator_enabled";
pub const DEFAULT_LIMIT: i64 = 20;
pub const MAX_LIMIT: i64 = 100;

/// Only a literal JSON boolean true opts in. Legacy malformed metadata is private.
pub fn is_enabled(metadata: &Value) -> bool {
    metadata.get(SPECTATOR_ENABLED).and_then(Value::as_bool) == Some(true)
}

/// Validate only the publication flag; preserve unrelated metadata semantics.
pub fn validate_metadata(metadata: &Value) -> Result<(), AppError> {
    if let Some(flag) = metadata.get(SPECTATOR_ENABLED)
        && !flag.is_boolean()
    {
        return Err(AppError::Validation(
            "metadata.spectator_enabled must be a boolean".into(),
        ));
    }
    Ok(())
}

/// Check every anonymous request before loading any project-owned content.
/// Missing, mismatched, encrypted, or unknown tenants fail closed, even if unlocked.
pub fn ensure_access(project: &Project, tenant: Option<&Tenant>) -> Result<(), AppError> {
    if !is_enabled(&project.metadata)
        || !tenant.is_some_and(|t| t.id == project.tenant_id && t.encryption_mode == "none")
    {
        return Err(AppError::NotFound("Spectator project not found".into()));
    }
    Ok(())
}

/// Projections are not authorization: callers must first run `ensure_access`
/// and verify that every child record belongs to the requested project.
#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicProject {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub spectator: bool,
}

impl From<&Project> for PublicProject {
    fn from(p: &Project) -> Self {
        Self {
            id: p.id,
            name: p.name.clone(),
            description: p.description.clone(),
            spectator: true,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicTask {
    pub id: Uuid,
    pub number: i64,
    pub title: String,
    pub kind: String,
    pub state: String,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
}

impl From<&Task> for PublicTask {
    fn from(t: &Task) -> Self {
        Self {
            id: t.id,
            number: t.number,
            title: t.title.clone(),
            kind: t.kind.clone(),
            state: t.state.clone(),
            created_at: t.created_at,
            updated_at: t.updated_at,
            completed_at: t.completed_at,
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicTaskDetail {
    #[serde(flatten)]
    pub task: PublicTask,
    pub spec: Option<String>,
    pub acceptance_criteria: Vec<String>,
}

impl From<&Task> for PublicTaskDetail {
    fn from(t: &Task) -> Self {
        Self {
            task: PublicTask::from(t),
            spec: t
                .context
                .get("spec")
                .and_then(Value::as_str)
                .map(str::to_owned),
            acceptance_criteria: text_items(t.context.get("acceptance_criteria")),
        }
    }
}

/// Publish text only, never nested objects or arbitrary JSON. A single string
/// becomes one item; malformed array elements are omitted.
fn text_items(value: Option<&Value>) -> Vec<String> {
    match value {
        Some(Value::String(s)) => vec![s.clone()],
        Some(Value::Array(items)) => items
            .iter()
            .filter_map(Value::as_str)
            .map(str::to_owned)
            .collect(),
        _ => Vec::new(),
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicWork {
    pub id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub status: String,
    pub work_type: String,
    pub success_criteria: Vec<String>,
}

impl From<&Work> for PublicWork {
    fn from(w: &Work) -> Self {
        Self {
            id: w.id,
            title: w.title.clone(),
            description: w.description.clone(),
            status: w.status.clone(),
            work_type: w.work_type.clone(),
            success_criteria: text_items(Some(&w.success_criteria)),
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicKnowledge {
    pub id: Uuid,
    pub title: String,
    pub category: String,
    pub content: String,
    pub tags: Vec<String>,
}

impl From<&Knowledge> for PublicKnowledge {
    fn from(k: &Knowledge) -> Self {
        Self {
            id: k.id,
            title: k.title.clone(),
            category: k.category.clone(),
            content: k.content.clone(),
            tags: k.tags.clone(),
        }
    }
}

#[derive(Debug, Serialize, utoipa::ToSchema)]
pub struct PublicDecision {
    pub id: Uuid,
    pub title: String,
    pub status: String,
    pub context: String,
    pub decision: Option<String>,
    pub rationale: Option<String>,
}

impl From<&Decision> for PublicDecision {
    fn from(d: &Decision) -> Self {
        Self {
            id: d.id,
            title: d.title.clone(),
            status: d.status.clone(),
            context: d.context.clone(),
            decision: d.decision.clone(),
            rationale: d.rationale.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn project() -> Project {
        serde_json::from_value(json!({
            "id": Uuid::nil(), "name": "Demo", "slug": "private-slug",
            "description": "Published description", "owner_id": Uuid::nil(),
            "default_branch": "private-branch", "repo_url": "private-repo",
            "metadata": {"spectator_enabled": true, "secret": "hidden"},
            "created_at": Utc::now(), "updated_at": Utc::now(),
            "git_mode": "none", "tenant_id": Uuid::nil()
        }))
        .unwrap()
    }

    fn tenant() -> Tenant {
        serde_json::from_value(json!({
            "id": Uuid::nil(), "name": "Private tenant", "slug": "private",
            "encryption_mode": "none", "theme_preference": "dark",
            "accent_color": "blue", "plan": "free", "rate_limit_per_min": 100,
            "max_tasks": 100, "max_projects": 100, "max_agents": 100,
            "created_at": Utc::now(), "updated_at": Utc::now()
        }))
        .unwrap()
    }

    fn task(context: Value) -> Task {
        serde_json::from_value(json!({
            "id": Uuid::nil(), "project_id": Uuid::nil(), "number": 1,
            "title": "Published title", "kind": "feature", "state": "ready",
            "urgent": true, "context": context, "required_capabilities": ["secret"],
            "created_by": Uuid::nil(), "created_at": Utc::now(), "updated_at": Utc::now(),
            "flagged": true, "file_scope": ["private-path"], "input_tokens": 42,
            "output_tokens": 42, "cost_usd": 1.0, "state_entered_at": Utc::now()
        }))
        .unwrap()
    }

    fn assert_keys(value: &Value, expected: &[&str]) {
        let mut keys: Vec<_> = value
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut expected = expected.to_vec();
        keys.sort();
        expected.sort();
        assert_eq!(keys, expected);
    }

    #[test]
    fn spectator_opt_in_is_strict() {
        for metadata in [
            json!({}),
            json!(null),
            json!([]),
            json!(true),
            json!({"spectator_enabled": false}),
        ] {
            assert!(!is_enabled(&metadata));
            assert!(validate_metadata(&metadata).is_ok());
        }
        for flag in [
            json!(null),
            json!("true"),
            json!(1),
            json!([]),
            json!({"enabled": true}),
        ] {
            let metadata = json!({"spectator_enabled": flag});
            assert!(!is_enabled(&metadata));
            assert!(validate_metadata(&metadata).is_err());
        }
        assert!(is_enabled(&json!({"spectator_enabled": true})));
    }

    #[test]
    fn spectator_policy_fails_closed_without_keys() {
        let mut p = project();
        let mut t = tenant();
        assert!(ensure_access(&p, Some(&t)).is_ok());
        assert!(ensure_access(&p, None).is_err());
        for mode in ["login_derived", "passphrase", "unknown", ""] {
            t.encryption_mode = mode.into();
            assert!(matches!(
                ensure_access(&p, Some(&t)),
                Err(AppError::NotFound(_))
            ));
        }
        t.encryption_mode = "none".into();
        t.id = Uuid::now_v7();
        assert!(ensure_access(&p, Some(&t)).is_err());
        t.id = p.tenant_id;
        for metadata in [
            json!({}),
            json!({"spectator_enabled": false}),
            json!({"spectator_enabled": "true"}),
        ] {
            p.metadata = metadata;
            assert!(ensure_access(&p, Some(&t)).is_err());
        }
    }

    #[test]
    fn spectator_project_and_task_projections_are_allowlisted() {
        let p = serde_json::to_value(PublicProject::from(&project())).unwrap();
        assert_keys(&p, &["id", "name", "description", "spectator"]);
        assert_eq!(p["spectator"], true);
        let t = task(
            json!({"spec": "Public spec", "acceptance_criteria": ["Public criterion", {"secret": "hidden"}],
            "notes": "hidden", "credentials": "hidden", "files": ["hidden"]}),
        );
        let summary = serde_json::to_value(PublicTask::from(&t)).unwrap();
        assert_keys(
            &summary,
            &[
                "id",
                "number",
                "title",
                "kind",
                "state",
                "created_at",
                "updated_at",
                "completed_at",
            ],
        );
        let detail = serde_json::to_value(PublicTaskDetail::from(&t)).unwrap();
        assert_keys(
            &detail,
            &[
                "id",
                "number",
                "title",
                "kind",
                "state",
                "created_at",
                "updated_at",
                "completed_at",
                "spec",
                "acceptance_criteria",
            ],
        );
        assert_eq!(detail["spec"], "Public spec");
        assert_eq!(detail["acceptance_criteria"], json!(["Public criterion"]));
        let malformed = PublicTaskDetail::from(&task(
            json!({"spec": {"secret": "hidden"}, "acceptance_criteria": {"secret": "hidden"}}),
        ));
        assert!(malformed.spec.is_none());
        assert!(malformed.acceptance_criteria.is_empty());
        assert_eq!(
            text_items(Some(&json!("Single criterion"))),
            vec!["Single criterion"]
        );
    }

    #[test]
    fn spectator_other_projections_are_allowlisted() {
        let now = Utc::now();
        let w = Work {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            title: "Work".into(),
            description: None,
            status: "active".into(),
            work_type: "feature".into(),
            parent_work_id: None,
            auto_status: false,
            intent_type: None,
            success_criteria: json!(["Public criterion", {"secret": "hidden"}]),
            metadata: json!({"secret": "hidden"}),
            sort_order: 0,
            created_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
        };
        let v = serde_json::to_value(PublicWork::from(&w)).unwrap();
        assert_keys(
            &v,
            &[
                "id",
                "title",
                "description",
                "status",
                "work_type",
                "success_criteria",
            ],
        );
        assert_eq!(v["success_criteria"], json!(["Public criterion"]));
        let k = Knowledge {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            title: "Knowledge".into(),
            category: "architecture".into(),
            content: "Public content".into(),
            tags: vec!["Public tag".into()],
            metadata: json!({"secret": "hidden"}),
            created_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
            embedding: Some(vec![1.0]),
        };
        let v = serde_json::to_value(PublicKnowledge::from(&k)).unwrap();
        assert_keys(&v, &["id", "title", "category", "content", "tags"]);
        assert_eq!(v["content"], "Public content");
        let d = Decision {
            id: Uuid::nil(),
            project_id: Uuid::nil(),
            title: "Decision".into(),
            status: "accepted".into(),
            context: "Public context".into(),
            decision: Some("Public decision".into()),
            rationale: Some("Public rationale".into()),
            alternatives: vec![],
            consequences: Some("hidden".into()),
            superseded_by: None,
            tags: vec!["hidden".into()],
            decided_by: Some(Uuid::nil()),
            created_by: Uuid::nil(),
            created_at: now,
            updated_at: now,
        };
        let v = serde_json::to_value(PublicDecision::from(&d)).unwrap();
        assert_keys(
            &v,
            &["id", "title", "status", "context", "decision", "rationale"],
        );
        assert_eq!(v["rationale"], "Public rationale");
    }

    #[test]
    fn spectator_validation_uses_existing_project_paths() {
        use crate::{
            models::{CreateProject, UpdateProject},
            validation::{validate_create_project, validate_update_project},
        };
        for metadata in [
            json!({}),
            json!({"spectator_enabled": true}),
            json!({"spectator_enabled": false}),
            json!({"unrelated": {"anything": [1, true]}}),
            json!(["legacy"]),
        ] {
            let c: CreateProject =
                serde_json::from_value(json!({"name": "Demo", "metadata": metadata})).unwrap();
            let u: UpdateProject = serde_json::from_value(json!({"metadata": metadata})).unwrap();
            assert!(validate_create_project(&c).is_ok());
            assert!(validate_update_project(&u).is_ok());
        }
        for flag in [json!(null), json!("true"), json!(1), json!([]), json!({})] {
            let c: CreateProject = serde_json::from_value(
                json!({"name": "Demo", "metadata": {"spectator_enabled": flag}}),
            )
            .unwrap();
            let u: UpdateProject =
                serde_json::from_value(json!({"metadata": {"spectator_enabled": flag}})).unwrap();
            assert!(matches!(
                validate_create_project(&c),
                Err(AppError::Validation(_))
            ));
            assert!(matches!(
                validate_update_project(&u),
                Err(AppError::Validation(_))
            ));
        }
    }
}
