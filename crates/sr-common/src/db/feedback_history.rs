use chrono::{DateTime, Utc};
use tracing::instrument;

use crate::api::models::queue::FeedbackEventRow;
use crate::db::util::TimedClientExt;
use crate::db::PgPool;

db_error!(FeedbackHistoryError {});

fn map_feedback_row(row: tokio_postgres::Row) -> FeedbackEventRow {
    FeedbackEventRow {
        id: row.get::<_, i64>("id"),
        interaction_id: row.get::<_, Option<i64>>("interaction_id"),
        match_result_id: row.get::<_, Option<i64>>("match_result_id"),
        match_run_id: row.get("match_run_id"),
        engine_version: row.get("engine_version"),
        config_version: row.get("config_version"),
        project_id: row.get::<_, i64>("project_id"),
        talent_id: row.get::<_, i64>("talent_id"),
        feedback_type: row.get("feedback_type"),
        ng_reason_category: row.get("ng_reason_category"),
        comment: row.get("comment"),
        actor: row.get("actor"),
        source: row.get("source"),
        is_revoked: row.get("is_revoked"),
        created_at: row.get::<_, DateTime<Utc>>("created_at"),
    }
}

/// interaction_id に紐づくフィードバック履歴を新しい順に取得する。
#[instrument(skip(pool))]
pub async fn fetch_feedback_history(
    pool: &PgPool,
    interaction_id: i64,
    limit: usize,
) -> Result<Vec<FeedbackEventRow>, FeedbackHistoryError> {
    let client = pool.get().await?;
    let limit_i64 = i64::try_from(limit.min(500)).unwrap_or(0);

    let stmt = client
        .prepare_cached(
            "SELECT id, interaction_id, match_result_id, match_run_id, engine_version, config_version, project_id, talent_id, feedback_type, ng_reason_category, comment, actor, source, is_revoked, created_at
             FROM ses.feedback_events
             WHERE interaction_id = $1
             ORDER BY created_at DESC
             LIMIT $2",
        )
        .await?;

    let rows = client
        .timed_query(
            &stmt,
            &[&interaction_id, &limit_i64],
            "fetch_feedback_history",
        )
        .await?;
    Ok(rows.into_iter().map(map_feedback_row).collect())
}
