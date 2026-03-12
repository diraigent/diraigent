use anyhow::{Context, Result};
use sqlx::{PgPool, postgres::PgPoolOptions};
use std::{env, time::Duration};

pub type DbPool = PgPool;

pub async fn create_pool() -> Result<DbPool, sqlx::Error> {
    let database_url = env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:postgres@localhost:5432/postgres".to_string());

    let pool = PgPoolOptions::new()
        .max_connections(20)
        .acquire_timeout(Duration::from_secs(8))
        .connect(&database_url)
        .await?;

    sqlx::query("SELECT 1").execute(&pool).await?;
    tracing::info!("✅ Database connection");

    Ok(pool)
}

pub struct SqlRepositoryBase {
    pub pool: DbPool,
}

impl SqlRepositoryBase {
    pub fn new(pool: DbPool) -> Self {
        Self { pool }
    }

    pub async fn ping(&self) -> Result<()> {
        sqlx::query("SELECT 1")
            .execute(&self.pool)
            .await
            .with_context(|| "Failed to ping database")?;
        Ok(())
    }

    pub fn pool(&self) -> &DbPool {
        &self.pool
    }
}
