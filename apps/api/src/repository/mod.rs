use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

mod agents;
mod audit;
mod decisions;
mod event_rules;
mod events;
mod files;
mod goals;
mod integrations;
mod knowledge;
mod memberships;
mod metrics;
mod observations;
mod packages;
mod plans;
mod playbooks;
mod projects;
mod reports;
mod roles;
mod search;
mod step_templates;
mod task_logs;
mod tasks;
mod tenants;
mod transitions;
mod verifications;
mod webhooks;

pub use agents::*;
pub use audit::*;
pub use decisions::*;
pub use event_rules::*;
pub use events::*;
pub use files::*;
pub use goals::*;
pub use integrations::*;
pub use knowledge::*;
pub use memberships::*;
pub use metrics::*;
pub use observations::*;
pub use packages::*;
pub use plans::*;
pub use playbooks::*;
pub use projects::*;
pub use reports::*;
pub use roles::*;
pub use search::*;
pub use step_templates::*;
pub use task_logs::*;
pub use tasks::*;
pub use tenants::*;
pub use transitions::*;
pub use verifications::*;
pub use webhooks::*;

// ── Shared utilities ──

/// Generate a new agent API key and its SHA-256 hash.
/// Returns (plaintext_key, hex_hash).
pub fn generate_agent_api_key() -> (String, String) {
    let random_bytes: [u8; 32] = rand::random();
    let key = format!("dak_{}", hex::encode(random_bytes));
    let hash = hex::encode(Sha256::digest(key.as_bytes()));
    (key, hash)
}

/// Hash an API key for lookup.
pub fn hash_api_key(key: &str) -> String {
    hex::encode(Sha256::digest(key.as_bytes()))
}

/// Whitelist of tables accessible via the generic `fetch_by_id` / `delete_by_id` helpers.
/// Using an enum instead of a raw `&str` prevents SQL injection by ensuring only
/// known table names can ever reach the query string.
#[derive(Clone, Copy)]
pub(crate) enum Table {
    Agent,
    ChangedFile,
    Decision,
    Event,
    EventObservationRule,
    Goal,
    Integration,
    Knowledge,
    Membership,
    Observation,
    Package,
    Plan,
    Playbook,
    Project,
    Report,
    Role,
    StepTemplate,
    Task,
    Tenant,
    TenantMember,
    Verification,
    Webhook,
    WrappedKey,
}

impl Table {
    fn as_str(self) -> &'static str {
        match self {
            Table::Agent => "agent",
            Table::ChangedFile => "task_changed_file",
            Table::Decision => "decision",
            Table::Event => "event",
            Table::EventObservationRule => "event_observation_rule",
            Table::Goal => "goal",
            Table::Integration => "integration",
            Table::Knowledge => "knowledge",
            Table::Membership => "membership",
            Table::Observation => "observation",
            Table::Package => "package",
            Table::Plan => "plan",
            Table::Playbook => "playbook",
            Table::Project => "project",
            Table::Report => "report",
            Table::Role => "role",
            Table::StepTemplate => "step_template",
            Table::Task => "task",
            Table::Tenant => "tenant",
            Table::TenantMember => "tenant_member",
            Table::Verification => "verification",
            Table::Webhook => "webhook",
            Table::WrappedKey => "wrapped_key",
        }
    }
}

pub(crate) fn slugify(name: &str) -> String {
    name.to_lowercase()
        .trim()
        .replace(|c: char| !c.is_alphanumeric() && c != '-' && c != ' ', "")
        .replace(' ', "-")
        .replace("--", "-")
}

pub(crate) async fn fetch_by_id<T>(
    pool: &PgPool,
    table: Table,
    id: Uuid,
    not_found_msg: &str,
) -> Result<T, AppError>
where
    T: for<'r> sqlx::FromRow<'r, sqlx::postgres::PgRow> + Unpin + Send,
{
    let table = table.as_str();
    sqlx::query_as::<_, T>(&format!("SELECT * FROM diraigent.{table} WHERE id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or_else(|| AppError::NotFound(not_found_msg.into()))
}

pub(crate) async fn delete_by_id(
    pool: &PgPool,
    table: Table,
    id: Uuid,
    not_found_msg: &str,
) -> Result<(), AppError> {
    let table = table.as_str();
    let result = sqlx::query(&format!("DELETE FROM diraigent.{table} WHERE id = $1"))
        .bind(id)
        .execute(pool)
        .await?;
    if result.rows_affected() == 0 {
        return Err(AppError::NotFound(not_found_msg.into()));
    }
    Ok(())
}
