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
    applied_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
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
}, Migration {
    id: 2,
    description: "soft delete + indexes for match_results, timestamp defaults, JSONB indexes",
    sql: r#"
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'ses' AND table_name = 'match_results'
    ) THEN
        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'ses'
              AND table_name = 'match_results'
              AND column_name = 'is_deleted'
        ) THEN
            ALTER TABLE ses.match_results ADD COLUMN is_deleted BOOLEAN NOT NULL DEFAULT false;
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'ses'
              AND table_name = 'match_results'
              AND column_name = 'deleted_at'
        ) THEN
            ALTER TABLE ses.match_results ADD COLUMN deleted_at TIMESTAMPTZ;
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM information_schema.columns
            WHERE table_schema = 'ses'
              AND table_name = 'match_results'
              AND column_name = 'deleted_by'
        ) THEN
            ALTER TABLE ses.match_results ADD COLUMN deleted_by TEXT;
        END IF;

        ALTER TABLE ses.match_results ALTER COLUMN created_at SET DEFAULT clock_timestamp();
        ALTER TABLE ses.match_results ALTER COLUMN updated_at SET DEFAULT clock_timestamp();

        IF EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'match_results_talent_id_project_id_run_date_key'
        ) THEN
            ALTER TABLE ses.match_results DROP CONSTRAINT match_results_talent_id_project_id_run_date_key;
        END IF;

        IF NOT EXISTS (
            SELECT 1 FROM pg_constraint WHERE conname = 'uniq_match_results_active'
        ) THEN
            ALTER TABLE ses.match_results
                ADD CONSTRAINT uniq_match_results_active UNIQUE (talent_id, project_id, run_date, last_match_run_id)
                WHERE deleted_at IS NULL;
        END IF;

        DROP INDEX IF EXISTS idx_match_results_talent_run_date;
        CREATE INDEX idx_match_results_talent_run_date
            ON ses.match_results(talent_id, run_date DESC)
            WHERE deleted_at IS NULL;

        DROP INDEX IF EXISTS idx_match_results_project_run_date;
        CREATE INDEX idx_match_results_project_run_date
            ON ses.match_results(project_id, run_date DESC)
            WHERE deleted_at IS NULL;

        DROP INDEX IF EXISTS idx_match_results_project_score_created;
        CREATE INDEX idx_match_results_project_score_created
            ON ses.match_results(project_id, score_total DESC, created_at DESC)
            WHERE deleted_at IS NULL;

        DROP INDEX IF EXISTS idx_match_results_score;
        CREATE INDEX idx_match_results_score
            ON ses.match_results(score_total DESC)
            WHERE NOT is_knockout AND deleted_at IS NULL;

        DROP INDEX IF EXISTS idx_match_results_match_run;
        CREATE INDEX idx_match_results_match_run
            ON ses.match_results(last_match_run_id)
            WHERE last_match_run_id IS NOT NULL AND deleted_at IS NULL;

        CREATE INDEX IF NOT EXISTS idx_match_results_score_breakdown_json
            ON ses.match_results USING GIN(score_breakdown jsonb_path_ops)
            WHERE score_breakdown IS NOT NULL AND deleted_at IS NULL;
    END IF;
END $$;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = 'ses' AND table_name = 'extraction_queue'
    ) THEN
        ALTER TABLE ses.extraction_queue ALTER COLUMN created_at SET DEFAULT clock_timestamp();
        ALTER TABLE ses.extraction_queue ALTER COLUMN updated_at SET DEFAULT clock_timestamp();
        CREATE INDEX IF NOT EXISTS idx_extraction_queue_partial_fields_json
            ON ses.extraction_queue USING GIN(partial_fields jsonb_path_ops);
    END IF;
END $$;

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'anken_emails') THEN
        ALTER TABLE ses.anken_emails ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'jinzai_emails') THEN
        ALTER TABLE ses.jinzai_emails ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'talents') THEN
        ALTER TABLE ses.talents ALTER COLUMN created_at SET DEFAULT clock_timestamp();
        ALTER TABLE ses.talents ALTER COLUMN updated_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'llm_comparison_results') THEN
        ALTER TABLE ses.llm_comparison_results ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'feedback_events') THEN
        ALTER TABLE ses.feedback_events ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'interaction_events') THEN
        ALTER TABLE ses.interaction_events ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'conversion_events') THEN
        ALTER TABLE ses.conversion_events ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    IF EXISTS (SELECT 1 FROM information_schema.tables WHERE table_schema = 'ses' AND table_name = 'interaction_logs') THEN
        ALTER TABLE ses.interaction_logs ALTER COLUMN created_at SET DEFAULT clock_timestamp();
    END IF;

    ALTER TABLE IF EXISTS ses.schema_migrations ALTER COLUMN applied_at SET DEFAULT clock_timestamp();
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
                applied_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()
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
