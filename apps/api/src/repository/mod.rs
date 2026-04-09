use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

mod account;
mod agents;
mod audit;
mod decisions;
mod event_rules;
mod events;
mod files;
mod forgejo;
mod github;
mod integrations;
mod knowledge;
mod memberships;
mod metrics;
mod observations;
mod packages;
mod playbooks;
mod projects;
mod provider_configs;
mod related;
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
mod work;

pub use account::*;
pub use agents::*;
pub use audit::*;
pub use decisions::*;
pub use event_rules::*;
pub use events::*;
pub use files::*;
pub use forgejo::*;
pub use github::*;
pub use integrations::*;
pub use knowledge::*;
pub use memberships::*;
pub use metrics::*;
pub use observations::*;
pub use packages::*;
pub use playbooks::*;
pub use projects::*;
pub use provider_configs::*;
pub use related::*;
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
pub use work::*;

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
    Work,
    Integration,
    Knowledge,
    Membership,
    Observation,
    Package,
    Project,
    ProviderConfig,
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
            Table::Work => "work",
            Table::Integration => "integration",
            Table::Knowledge => "knowledge",
            Table::Membership => "membership",
            Table::Observation => "observation",
            Table::Package => "package",
            Table::Project => "project",
            Table::ProviderConfig => "provider_config",
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

/// Generate `list_*` and `count_*` functions for a filtered, paginated query.
///
/// Usage:
/// ```ignore
/// list_and_count!(
///     list_events, count_events,    // function names
///     Event, EventFilters,          // return type, filter type
///     "event", EVENT_FILTERS_WHERE, // table name, WHERE constant
///     |filters| { filters.limit },  // limit accessor
///     |filters| { filters.offset }, // offset accessor
///     |q, filters| {                // bind filter params (excluding limit/offset)
///         q.bind(&filters.kind).bind(&filters.severity).bind(filters.since)
///     }
/// );
/// ```
macro_rules! list_and_count {
    (
        $list_fn:ident, $count_fn:ident,
        $row_type:ty, $filters_type:ty,
        $table:expr, $where_clause:expr,
        |$lf:ident| $limit_expr:expr,
        |$of:ident| $offset_expr:expr,
        |$q:ident, $f:ident| $bind_expr:expr
    ) => {
        pub async fn $list_fn(
            pool: &sqlx::PgPool,
            project_id: uuid::Uuid,
            filters: &$filters_type,
        ) -> Result<Vec<$row_type>, $crate::error::AppError> {
            let $lf = filters;
            let limit = ($limit_expr).unwrap_or(50).min(100);
            let $of = filters;
            let offset = ($offset_expr).unwrap_or(0);

            // Count filter binds to compute correct limit/offset parameter positions
            let filter_sql = $where_clause;
            let max_param = filter_sql
                .match_indices('$')
                .filter_map(|(i, _)| {
                    filter_sql[i + 1..]
                        .chars()
                        .take_while(|c| c.is_ascii_digit())
                        .collect::<String>()
                        .parse::<i32>()
                        .ok()
                })
                .max()
                .unwrap_or(0);
            let sql = format!(
                "SELECT * FROM diraigent.{} {} ORDER BY created_at DESC LIMIT ${} OFFSET ${}",
                $table,
                filter_sql,
                max_param + 1,
                max_param + 2,
            );

            let $q = sqlx::query_as::<_, $row_type>(&sql).bind(project_id);
            let $f = filters;
            let query = $bind_expr;
            let items = query.bind(limit).bind(offset).fetch_all(pool).await?;
            Ok(items)
        }

        pub async fn $count_fn(
            pool: &sqlx::PgPool,
            project_id: uuid::Uuid,
            filters: &$filters_type,
        ) -> Result<i64, $crate::error::AppError> {
            let sql = format!(
                "SELECT COUNT(*) FROM diraigent.{} {}",
                $table, $where_clause,
            );
            let $q = sqlx::query_as::<_, (i64,)>(&sql).bind(project_id);
            let $f = filters;
            let query = $bind_expr;
            let row = query.fetch_one(pool).await?;
            Ok(row.0)
        }
    };
}

pub(crate) use list_and_count;

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
