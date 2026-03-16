use sqlx::PgPool;
use uuid::Uuid;

use crate::error::AppError;

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
