use chrono::{DateTime, Utc};
use serde_json::Value;
use tracing::instrument;

use crate::db::util::TimedClientExt;
use crate::db::{normalize_json, PgPool};

db_error!(MatchResultStorageError {});

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
    /// 実行インスタンスID（ULID/UUID、毎回生成）
    pub match_run_id: Option<String>,
    pub created_at: Option<DateTime<Utc>>,
}

/// Insert or update a match result snapshot.
/// Same-day duplicates are updated with the latest values (UPSERT pattern).
/// The `run_date` column is a generated column computed from `created_at` in JST timezone.
#[instrument(skip(pool, result))]
pub async fn insert_match_result(
    pool: &PgPool,
    result: &MatchResultInsert,
) -> Result<u64, MatchResultStorageError> {
    let client = pool.get().await?;

    // run_date is a GENERATED ALWAYS column computed from created_at in JST timezone.
    // We don't pass it explicitly - PostgreSQL computes it automatically.
    // ON CONFLICT updates the existing record with latest values (same JST day = same snapshot).
    // Note: EXCLUDED.created_at is within the same JST day, so run_date won't change.
    let stmt = client
        .prepare_cached(
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
                last_match_run_id,
                created_at,
                updated_at
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $11
            )
            ON CONFLICT ON CONSTRAINT uniq_match_results_active DO UPDATE SET
                is_knockout = EXCLUDED.is_knockout,
                ko_reasons = EXCLUDED.ko_reasons,
                needs_manual_review = EXCLUDED.needs_manual_review,
                score_total = EXCLUDED.score_total,
                score_breakdown = EXCLUDED.score_breakdown,
                engine_version = EXCLUDED.engine_version,
                rule_version = EXCLUDED.rule_version,
                last_match_run_id = EXCLUDED.last_match_run_id,
                updated_at = EXCLUDED.updated_at,
                is_deleted = false,
                deleted_at = NULL,
                deleted_by = NULL",
        )
        .await?;

    let now = Utc::now();
    let created_at = result.created_at.unwrap_or(now);
    let rows = client
        .timed_execute(
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
                &result.match_run_id,
                &created_at,
            ],
            "insert_match_result",
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
        assert!(insert.match_run_id.is_none());
    }

    #[test]
    fn accepts_match_run_id() {
        let insert = MatchResultInsert {
            talent_id: 1,
            project_id: 2,
            is_knockout: false,
            needs_manual_review: false,
            match_run_id: Some("01HXYZ123456".to_string()),
            ..Default::default()
        };

        assert_eq!(insert.match_run_id.as_deref(), Some("01HXYZ123456"));
    }
}
