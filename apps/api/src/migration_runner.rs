//! Apply immutable migrations, including compatibility with the historical
//! schema snapshot's membership constraint name before migration 042.
use sqlx::{
    PgPool,
    migrate::{Migrate, MigrateError, Migrator},
};
use std::borrow::Cow;

pub static MIGRATIONS: Migrator = sqlx::migrate!("./migrations");

pub async fn run(pool: &PgPool) -> Result<(), MigrateError> {
    let mut conn = pool.acquire().await?;
    conn.lock().await?;
    let result = run_locked(&mut conn).await;
    let unlock = conn.unlock().await;
    result?;
    unlock
}

async fn run_locked(conn: &mut sqlx::PgConnection) -> Result<(), MigrateError> {
    let prefix = Migrator {
        migrations: Cow::Owned(
            MIGRATIONS
                .iter()
                .filter(|m| m.version < 42)
                .cloned()
                .collect(),
        ),
        ignore_missing: true,
        locking: false,
        ..Migrator::DEFAULT
    };
    prefix.run_direct(conn).await?;
    // 001's snapshot already has the final name, while 042 expects the old
    // name. Normalize only when 042 is pending; its original SQL/checksum still
    // runs unchanged. Already-upgraded installations never enter this branch.
    sqlx::raw_sql(r#"DO $$ BEGIN
        IF NOT EXISTS (SELECT 1 FROM _sqlx_migrations WHERE version = 42 AND success)
           AND EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'diraigent.membership'::regclass AND conname = 'membership_tenant_agent_role_key')
           AND NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conrelid = 'diraigent.membership'::regclass AND conname = 'membership_agent_id_role_id_key') THEN
            ALTER TABLE diraigent.membership RENAME CONSTRAINT membership_tenant_agent_role_key TO membership_agent_id_role_id_key;
        END IF;
    END $$;"#).execute(&mut *conn).await?;
    let all = Migrator {
        migrations: Cow::Borrowed(MIGRATIONS.migrations.as_ref()),
        locking: false,
        ..Migrator::DEFAULT
    };
    all.run_direct(conn).await
}
