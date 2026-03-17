use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

/// Export all data associated with a user as a JSON value.
///
/// Collects data from every table that references the user's UUID,
/// grouped by category. This is the GDPR "right of access" / data portability
/// export.
pub async fn export_user_data(pool: &PgPool, user_id: Uuid) -> Result<serde_json::Value, AppError> {
    // Helper: run a query and return rows as JSON arrays
    macro_rules! export_table {
        ($query:expr) => {{
            let rows: Vec<serde_json::Value> = sqlx::query_scalar($query)
                .bind(user_id)
                .fetch_all(pool)
                .await?;
            serde_json::Value::Array(rows)
        }};
    }

    let account =
        export_table!("SELECT to_jsonb(a) FROM diraigent.auth_user a WHERE a.user_id = $1");

    let tenant_memberships =
        export_table!("SELECT to_jsonb(tm) FROM diraigent.tenant_member tm WHERE tm.user_id = $1");

    let tenants = export_table!(
        "SELECT to_jsonb(t) FROM diraigent.tenant t \
         WHERE t.id IN (SELECT tenant_id FROM diraigent.tenant_member WHERE user_id = $1)"
    );

    let agents = export_table!("SELECT to_jsonb(a) FROM diraigent.agent a WHERE a.owner_id = $1");

    let projects = export_table!(
        "SELECT to_jsonb(p) FROM diraigent.project p \
         WHERE p.tenant_id IN (SELECT tenant_id FROM diraigent.tenant_member WHERE user_id = $1)"
    );

    let tasks_created =
        export_table!("SELECT to_jsonb(t) FROM diraigent.task t WHERE t.created_by = $1");

    let task_comments =
        export_table!("SELECT to_jsonb(c) FROM diraigent.task_comment c WHERE c.user_id = $1");

    let task_updates =
        export_table!("SELECT to_jsonb(u) FROM diraigent.task_update u WHERE u.user_id = $1");

    let work_items_created =
        export_table!("SELECT to_jsonb(w) FROM diraigent.work w WHERE w.created_by = $1");

    let work_comments =
        export_table!("SELECT to_jsonb(c) FROM diraigent.work_comment c WHERE c.user_id = $1");

    let decisions = export_table!(
        "SELECT to_jsonb(d) FROM diraigent.decision d \
         WHERE d.created_by = $1 OR d.decided_by = $1"
    );

    let knowledge =
        export_table!("SELECT to_jsonb(k) FROM diraigent.knowledge k WHERE k.created_by = $1");

    let verifications =
        export_table!("SELECT to_jsonb(v) FROM diraigent.verification v WHERE v.user_id = $1");

    let playbooks =
        export_table!("SELECT to_jsonb(p) FROM diraigent.playbook p WHERE p.created_by = $1");

    let step_templates =
        export_table!("SELECT to_jsonb(s) FROM diraigent.step_template s WHERE s.created_by = $1");

    let reports =
        export_table!("SELECT to_jsonb(r) FROM diraigent.report r WHERE r.created_by = $1");

    let audit_log = export_table!(
        "SELECT to_jsonb(a) FROM diraigent.audit_log a WHERE a.actor_user_id = $1 \
         ORDER BY a.created_at DESC LIMIT 1000"
    );

    Ok(serde_json::json!({
        "exported_at": chrono::Utc::now().to_rfc3339(),
        "user_id": user_id,
        "account": account,
        "tenant_memberships": tenant_memberships,
        "tenants": tenants,
        "agents": agents,
        "projects": projects,
        "tasks_created": tasks_created,
        "task_comments": task_comments,
        "task_updates": task_updates,
        "work_items_created": work_items_created,
        "work_comments": work_comments,
        "decisions": decisions,
        "knowledge": knowledge,
        "verifications": verifications,
        "playbooks": playbooks,
        "step_templates": step_templates,
        "reports": reports,
        "audit_log": audit_log,
    }))
}

/// Delete a user account and nullify all references.
///
/// Order of operations:
/// 1. Nullify `created_by` / `owner_id` / `user_id` columns that reference
///    the user's UUID (these have no FK constraint but we clean them up).
/// 2. Delete the `auth_user` row — cascades `tenant_member` and `wrapped_key`.
///    The migration 038 also sets `agent.owner_id` to NULL on cascade.
pub async fn delete_user_account(pool: &PgPool, user_id: Uuid) -> Result<(), AppError> {
    let mut tx = pool.begin().await?;

    // ── Nullify non-FK references ─────────────────────────────────────────

    // audit_log.actor_user_id (nullable, no FK)
    sqlx::query("UPDATE diraigent.audit_log SET actor_user_id = NULL WHERE actor_user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // task_update.user_id (nullable, no FK)
    sqlx::query("UPDATE diraigent.task_update SET user_id = NULL WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // task_comment.user_id (nullable, no FK)
    sqlx::query("UPDATE diraigent.task_comment SET user_id = NULL WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // verification.user_id (nullable, no FK)
    sqlx::query("UPDATE diraigent.verification SET user_id = NULL WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // agent.owner_id — also handled by FK cascade (migration 038) but we
    // nullify explicitly so it works even before the migration runs.
    sqlx::query("UPDATE diraigent.agent SET owner_id = NULL WHERE owner_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    // ── Delete the auth_user row ──────────────────────────────────────────
    // This cascades: tenant_member, wrapped_key
    let result = sqlx::query("DELETE FROM diraigent.auth_user WHERE user_id = $1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

    if result.rows_affected() == 0 {
        return Err(AppError::NotFound("User not found".into()));
    }

    tx.commit().await?;
    Ok(())
}

/// Look up an auth_user by their external auth_user_id (e.g. Authentik sub).
/// Returns the internal user_id if found.
pub async fn get_user_id_by_auth_id(
    pool: &PgPool,
    auth_user_id: &str,
) -> Result<Option<Uuid>, AppError> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT user_id FROM diraigent.auth_user WHERE auth_user_id = $1")
            .bind(auth_user_id)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|r| r.0))
}
