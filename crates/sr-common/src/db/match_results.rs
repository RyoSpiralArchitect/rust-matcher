use chrono::{DateTime, Utc};
use deadpool_postgres::PoolError;
use serde_json::Value;
use tokio_postgres::Error as PgError;
use tokio_postgres::types::Json;
use tracing::instrument;

use crate::db::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum MatchResultStorageError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
}

#[derive(Debug, Clone, Default)]
pub struct MatchResultInsert {
    pub talent_id: i64,
    pub project_id: i64,
    pub is_knockout: bool,
    pub ko_reasons: Option<Value>,
    pub needs_manual_review: bool,
    pub score_total: Option<f64>,
    pub score_breakdown: Option<Value>,
    pub engine_version: Option<String>,
    pub rule_version: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

fn normalize_json(value: &Option<Value>) -> Option<Json<&Value>> {
    value.as_ref().map(Json)
}

/// Insert a match result snapshot. Duplicates for the same day are ignored.
#[instrument(skip(pool, result))]
pub async fn insert_match_result(
    pool: &PgPool,
    result: &MatchResultInsert,
) -> Result<u64, MatchResultStorageError> {
    let client = pool.get().await?;

    let stmt = client
        .prepare(
            "INSERT INTO ses.match_results (
                talent_id,
                project_id,
                is_knockout,
                ko_reasons,
                needs_manual_review,
                score_total,
                score_breakdown,
                engine_version,
                rule_version,
                created_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10
            )
            ON CONFLICT DO NOTHING;",
        )
        .await?;

    let created_at = result.created_at.unwrap_or_else(Utc::now);
    let rows = client
        .execute(
            &stmt,
            &[
                &result.talent_id,
                &result.project_id,
                &result.is_knockout,
                &normalize_json(&result.ko_reasons),
                &result.needs_manual_review,
                &result.score_total,
                &normalize_json(&result.score_breakdown),
                &result.engine_version,
                &result.rule_version,
                &created_at,
            ],
        )
        .await?;

    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_json_handles_options() {
        let none: Option<Value> = None;
        assert!(normalize_json(&none).is_none());

        let some = Some(serde_json::json!({"score": 0.9}));
        let normalized = normalize_json(&some);
        assert!(normalized.is_some());
    }

    #[test]
    fn defaults_created_at_when_missing() {
        let insert = MatchResultInsert {
            talent_id: 1,
            project_id: 2,
            is_knockout: false,
            needs_manual_review: false,
            ..Default::default()
        };

        assert!(insert.created_at.is_none());
    }
}
