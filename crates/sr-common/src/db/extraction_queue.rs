use chrono::{DateTime, Duration, Utc};
use deadpool_postgres::PoolError;
use serde_json::Value;
use tokio_postgres::Error as PgError;
use tokio_postgres::types::Json;
use tracing::instrument;

use crate::db::PgPool;
use crate::queue::{ExtractionJob, QueueStatus};

#[derive(Debug, thiserror::Error)]
pub enum QueueStorageError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
}

fn normalize_json(value: &Option<Value>) -> Option<Json<&Value>> {
    value.as_ref().map(Json)
}

/// Insert or update a queue row based on `message_id`.
#[instrument(skip(pool, job))]
pub async fn upsert_extraction_job(
    pool: &PgPool,
    job: &ExtractionJob,
) -> Result<u64, QueueStorageError> {
    let client = pool.get().await?;

    let stmt = client
        .prepare(
            "INSERT INTO ses.extraction_queue (
                message_id,
                email_subject,
                email_received_at,
                subject_hash,
                status,
                priority,
                locked_by,
                retry_count,
                next_retry_at,
                last_error,
                partial_fields,
                decision_reason,
                recommended_method,
                final_method,
                extractor_version,
                rule_version,
                created_at,
                processing_started_at,
                completed_at,
                updated_at,
                llm_latency_ms,
                requires_manual_review,
                manual_review_reason,
                reprocess_after,
                canary_target
            ) VALUES (
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10,
                $11, $12, $13, $14, $15, $16, $17, $18, $19, $20,
                $21, $22, $23, $24, $25
            )
            ON CONFLICT (message_id) DO UPDATE SET
                email_subject = EXCLUDED.email_subject,
                email_received_at = EXCLUDED.email_received_at,
                subject_hash = EXCLUDED.subject_hash,
                status = EXCLUDED.status,
                priority = EXCLUDED.priority,
                locked_by = EXCLUDED.locked_by,
                retry_count = EXCLUDED.retry_count,
                next_retry_at = EXCLUDED.next_retry_at,
                last_error = EXCLUDED.last_error,
                partial_fields = EXCLUDED.partial_fields,
                decision_reason = EXCLUDED.decision_reason,
                recommended_method = EXCLUDED.recommended_method,
                final_method = EXCLUDED.final_method,
                extractor_version = EXCLUDED.extractor_version,
                rule_version = EXCLUDED.rule_version,
                processing_started_at = EXCLUDED.processing_started_at,
                completed_at = EXCLUDED.completed_at,
                updated_at = EXCLUDED.updated_at,
                llm_latency_ms = EXCLUDED.llm_latency_ms,
                requires_manual_review = EXCLUDED.requires_manual_review,
                manual_review_reason = EXCLUDED.manual_review_reason,
                reprocess_after = EXCLUDED.reprocess_after,
                canary_target = EXCLUDED.canary_target;",
        )
        .await?;

    let recommended = job.recommended_method.as_ref().map(|r| r.as_str());
    let final_method = job.final_method.as_ref().map(|f| f.as_str());

    let rows = client
        .execute(
            &stmt,
            &[
                &job.message_id,
                &job.email_subject,
                &job.email_received_at,
                &job.subject_hash,
                &job.status.as_str(),
                &job.priority,
                &job.locked_by,
                &i32::try_from(job.retry_count).unwrap_or(i32::MAX),
                &job.next_retry_at,
                &job.last_error,
                &normalize_json(&job.partial_fields),
                &job.decision_reason,
                &recommended,
                &final_method,
                &job.extractor_version,
                &job.rule_version,
                &job.created_at,
                &job.processing_started_at,
                &job.completed_at,
                &job.updated_at,
                &job.llm_latency_ms,
                &job.requires_manual_review,
                &job.manual_review_reason,
                &job.reprocess_after,
                &job.canary_target,
            ],
        )
        .await?;

    Ok(rows)
}

/// Reset long-running processing jobs back to pending.
#[instrument(skip(pool))]
pub async fn recover_stuck_jobs(
    pool: &PgPool,
    now: DateTime<Utc>,
    max_processing: Duration,
) -> Result<u64, QueueStorageError> {
    let client = pool.get().await?;
    let cutoff = now - max_processing;

    let stmt = client
        .prepare(
            "UPDATE ses.extraction_queue SET
                status = 'pending',
                locked_by = NULL,
                next_retry_at = $1,
                updated_at = $1
            WHERE status = 'processing'
              AND COALESCE(processing_started_at, updated_at) <= $2",
        )
        .await?;

    let rows = client.execute(&stmt, &[&now, &cutoff]).await?;
    Ok(rows)
}

/// Return a safe pending copy for enqueueing without leaking processing metadata.
pub fn pending_copy(job: &ExtractionJob, received_at: DateTime<Utc>) -> ExtractionJob {
    let mut pending = job.clone();
    pending.status = QueueStatus::Pending;
    pending.retry_count = 0;
    pending.locked_by = None;
    pending.next_retry_at = None;
    pending.last_error = None;
    pending.final_method = None;
    pending.processing_started_at = None;
    pending.completed_at = None;
    pending.llm_latency_ms = None;
    pending.reprocess_after = None;
    pending.email_received_at = received_at;
    pending.updated_at = Utc::now();
    pending
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::queue::{FinalMethod, RecommendedMethod};

    fn sample_job() -> ExtractionJob {
        let mut job = ExtractionJob::new("m1", "s", Utc::now(), "hash");
        job.status = QueueStatus::Processing;
        job.retry_count = 2;
        job.locked_by = Some("worker".into());
        job.next_retry_at = Some(Utc::now());
        job.last_error = Some("oops".into());
        job.recommended_method = Some(RecommendedMethod::RustRecommended);
        job.final_method = Some(FinalMethod::RustCompleted);
        job.processing_started_at = Some(Utc::now());
        job.completed_at = Some(Utc::now());
        job.llm_latency_ms = Some(123);
        job.reprocess_after = Some(Utc::now());
        job.partial_fields = Some(serde_json::json!({"k":"v"}));
        job.requires_manual_review = true;
        job.manual_review_reason = Some("check".into());
        job
    }

    #[test]
    fn pending_copy_resets_runtime_fields() {
        let job = sample_job();
        let received = Utc::now();

        let pending = pending_copy(&job, received);
        assert_eq!(pending.status, QueueStatus::Pending);
        assert_eq!(pending.retry_count, 0);
        assert!(pending.locked_by.is_none());
        assert!(pending.next_retry_at.is_none());
        assert!(pending.last_error.is_none());
        assert!(pending.final_method.is_none());
        assert!(pending.processing_started_at.is_none());
        assert!(pending.completed_at.is_none());
        assert!(pending.llm_latency_ms.is_none());
        assert!(pending.reprocess_after.is_none());
        assert_eq!(pending.email_received_at, received);
        assert_eq!(pending.manual_review_reason, Some("check".into()));
        assert_eq!(pending.requires_manual_review, true);
        assert_eq!(
            pending.recommended_method,
            Some(RecommendedMethod::RustRecommended)
        );
        assert!(pending.updated_at >= job.updated_at);
    }

    #[test]
    fn normalize_json_handles_none_and_some() {
        let with_none: Option<Value> = None;
        assert!(normalize_json(&with_none).is_none());

        let with_value = Some(serde_json::json!({"a": 1}));
        let normalized = normalize_json(&with_value);
        assert!(normalized.is_some());
    }
}
