use chrono::{DateTime, Utc};
use tracing::instrument;

use crate::db::util::TimedClientExt;
use crate::db::PgPool;

db_error!(InteractionLogStorageError {});

#[derive(Debug, Clone, Default)]
pub struct InteractionLogInsert {
    pub match_result_id: Option<i64>,
    pub talent_id: i64,
    pub project_id: i64,
    pub match_run_id: String,
    pub engine_version: Option<String>,
    pub config_version: Option<String>,
    pub two_tower_score: Option<f64>,
    pub two_tower_embedder: Option<String>,
    pub two_tower_version: Option<String>,
    pub business_score: Option<f64>,
    pub outcome: Option<String>,
    pub feedback_at: Option<DateTime<Utc>>,
    pub variant: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Insert an interaction log row.
///
/// Uses the `(match_run_id, talent_id, project_id)` unique constraint to avoid
/// duplicate inserts within the same run. Returns the number of rows affected.
#[instrument(skip(pool, log))]
pub async fn insert_interaction_log(
    pool: &PgPool,
    log: &InteractionLogInsert,
) -> Result<u64, InteractionLogStorageError> {
    let client = pool.get().await?;

    let stmt = client
        .prepare_cached(
            "INSERT INTO ses.interaction_logs (
                match_result_id,
                talent_id,
                project_id,
                match_run_id,
                engine_version,
                config_version,
                two_tower_score,
                two_tower_embedder,
                two_tower_version,
                business_score,
                outcome,
                feedback_at,
                variant,
                created_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14
            )
            ON CONFLICT (match_run_id, talent_id, project_id) DO UPDATE SET
                match_result_id = COALESCE(EXCLUDED.match_result_id, ses.interaction_logs.match_result_id),
                engine_version = COALESCE(EXCLUDED.engine_version, ses.interaction_logs.engine_version),
                config_version = COALESCE(EXCLUDED.config_version, ses.interaction_logs.config_version),
                two_tower_score = COALESCE(EXCLUDED.two_tower_score, ses.interaction_logs.two_tower_score),
                two_tower_embedder = COALESCE(EXCLUDED.two_tower_embedder, ses.interaction_logs.two_tower_embedder),
                two_tower_version = COALESCE(EXCLUDED.two_tower_version, ses.interaction_logs.two_tower_version),
                business_score = COALESCE(EXCLUDED.business_score, ses.interaction_logs.business_score),
                variant = COALESCE(EXCLUDED.variant, ses.interaction_logs.variant)",
        )
        .await?;

    let created_at = log.created_at.unwrap_or_else(Utc::now);
    let rows = client
        .timed_execute(
            &stmt,
            &[
                &log.match_result_id,
                &log.talent_id,
                &log.project_id,
                &log.match_run_id,
                &log.engine_version,
                &log.config_version,
                &log.two_tower_score,
                &log.two_tower_embedder,
                &log.two_tower_version,
                &log.business_score,
                &log.outcome,
                &log.feedback_at,
                &log.variant,
                &created_at,
            ],
            "insert_interaction_log",
        )
        .await?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn interaction_log_insert_defaults_are_optional() {
        let insert = InteractionLogInsert::default();
        assert!(insert.match_result_id.is_none());
        assert!(insert.engine_version.is_none());
        assert!(insert.config_version.is_none());
        assert!(insert.two_tower_score.is_none());
        assert!(insert.two_tower_embedder.is_none());
        assert!(insert.two_tower_version.is_none());
        assert!(insert.business_score.is_none());
        assert!(insert.outcome.is_none());
        assert!(insert.feedback_at.is_none());
        assert!(insert.variant.is_none());
        assert!(insert.created_at.is_none());
    }
}
