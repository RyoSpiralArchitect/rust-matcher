use deadpool_postgres::PoolError;
use thiserror::Error;
use tokio_postgres::Error as PgError;
use tracing::{info, instrument};

use crate::db::{DbPoolError, PgPool};

#[derive(Debug, Error)]
pub enum MigrationError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("failed to run migration: {0}")]
    Postgres(#[from] PgError),
    #[error("failed to build pool: {0}")]
    PoolBuild(#[from] DbPoolError),
}

struct Migration {
    id: i32,
    description: &'static str,
    sql: &'static str,
}

const MIGRATIONS: &[Migration] = &[Migration {
    id: 1,
    description: "safety checks for queue status + score ranges",
    sql: r#"
CREATE SCHEMA IF NOT EXISTS ses;

CREATE TABLE IF NOT EXISTS ses.schema_migrations (
    id INTEGER PRIMARY KEY,
    description TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'ses' AND table_name = 'extraction_queue'
    ) THEN
        CREATE INDEX IF NOT EXISTS idx_extraction_queue_pending
            ON ses.extraction_queue(created_at, id)
            WHERE status = 'pending';
        CREATE INDEX IF NOT EXISTS idx_extraction_queue_status_created
            ON ses.extraction_queue(status, created_at, id);

        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'chk_retry_count'
        ) THEN
            ALTER TABLE ses.extraction_queue
                ADD CONSTRAINT chk_retry_count
                CHECK (retry_count >= 0 AND retry_count <= 100);
        END IF;
    END IF;
END $$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'ses' AND table_name = 'match_results'
    ) THEN
        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'chk_score_total_range'
        ) THEN
            ALTER TABLE ses.match_results
                ADD CONSTRAINT chk_score_total_range
                CHECK (score_total IS NULL OR (score_total >= 0.0 AND score_total <= 1.0));
        END IF;
    END IF;
END $$;
"#,
}];

#[instrument(skip(pool))]
pub async fn run_migrations(pool: &PgPool) -> Result<(), MigrationError> {
    let mut client = pool.get().await?;
    client
        .batch_execute(
            "CREATE SCHEMA IF NOT EXISTS ses;
             CREATE TABLE IF NOT EXISTS ses.schema_migrations (
                id INTEGER PRIMARY KEY,
                description TEXT NOT NULL,
                applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
             );",
        )
        .await?;

    for migration in MIGRATIONS {
        let already_applied: bool = client
            .query_one(
                "SELECT EXISTS (SELECT 1 FROM ses.schema_migrations WHERE id = $1)",
                &[&migration.id],
            )
            .await?
            .get(0);

        if already_applied {
            continue;
        }

        let tx = client.transaction().await?;
        tx.batch_execute(migration.sql).await?;
        tx.execute(
            "INSERT INTO ses.schema_migrations (id, description) VALUES ($1, $2)",
            &[&migration.id, &migration.description],
        )
        .await?;
        tx.commit().await?;

        info!(
            id = migration.id,
            description = migration.description,
            "applied migration"
        );
    }

    Ok(())
}
