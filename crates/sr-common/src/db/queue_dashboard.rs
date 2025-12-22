use chrono::{DateTime, Utc};
use deadpool_postgres::PoolError;
use tokio_postgres::Error as PgError;
use tokio_postgres::Row;
use tracing::instrument;

use crate::api::queue_dashboard::{QueueDashboard, QueueJobDetail, QueueJobSummary, StatusCounts};
use crate::db::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum QueueDashboardError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
}

fn map_job_summary(row: &Row) -> QueueJobSummary {
    QueueJobSummary {
        message_id: row.get("message_id"),
        subject: row.get("email_subject"),
        status: row.get("status"),
        recommended_method: row.get("recommended_method"),
        final_method: row.get("final_method"),
        locked_by: row.get("locked_by"),
        processing_started_at: row.get("processing_started_at"),
        last_error: row.get("last_error"),
        decision_reason: row.get("decision_reason"),
        requires_manual_review: row.get("requires_manual_review"),
        retry_count: row.get("retry_count"),
        received_at: row.get("email_received_at"),
        updated_at: row.get("updated_at"),
    }
}

#[instrument(skip(pool))]
pub async fn fetch_dashboard(pool: &PgPool) -> Result<QueueDashboard, QueueDashboardError> {
    let client = pool.get().await?;

    let row = client
        .query_one(
            "SELECT \
                COUNT(*) FILTER (WHERE status = 'pending') AS pending,\
                COUNT(*) FILTER (WHERE status = 'processing') AS processing,\
                COUNT(*) FILTER (WHERE status = 'completed') AS completed,\
                COUNT(*) FILTER (WHERE requires_manual_review) AS manual_review_count,\
                COUNT(*) FILTER (WHERE last_error IS NOT NULL) AS error_count,\
                COUNT(*) FILTER (\
                    WHERE status = 'processing'\
                      AND processing_started_at <= NOW() - INTERVAL '10 minutes'\
                ) AS stale_processing_count,\
                COALESCE(MAX(updated_at), NOW()) AS updated_at\
            FROM ses.extraction_queue",
            &[],
        )
        .await?;

    Ok(QueueDashboard {
        status_counts: StatusCounts {
            pending: row.get("pending"),
            processing: row.get("processing"),
            completed: row.get("completed"),
        },
        manual_review_count: row.get("manual_review_count"),
        error_count: row.get("error_count"),
        stale_processing_count: row.get("stale_processing_count"),
        updated_at: row.get("updated_at"),
    })
}

#[instrument(skip(pool))]
pub async fn fetch_jobs(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<QueueJobSummary>, QueueDashboardError> {
    let client = pool.get().await?;

    let rows = client
        .query(
            "SELECT\
                message_id,\
                email_subject,\
                status,\
                recommended_method,\
                final_method,\
                locked_by,\
                processing_started_at,\
                last_error,\
                decision_reason,\
                requires_manual_review,\
                retry_count,\
                email_received_at,\
                updated_at\
            FROM ses.extraction_queue\
            ORDER BY updated_at DESC\
            LIMIT $1",
            &[&limit],
        )
        .await?;

    Ok(rows.iter().map(map_job_summary).collect())
}

fn map_job_detail(row: &Row) -> QueueJobDetail {
    QueueJobDetail {
        summary: map_job_summary(row),
        partial_fields: row.get("partial_fields"),
        priority: row.get("priority"),
        llm_latency_ms: row.get("llm_latency_ms"),
        manual_review_reason: row.get("manual_review_reason"),
        created_at: row.get::<_, DateTime<Utc>>("created_at"),
        completed_at: row.get("completed_at"),
    }
}

#[instrument(skip(pool))]
pub async fn fetch_job_detail(
    pool: &PgPool,
    message_id: &str,
) -> Result<Option<QueueJobDetail>, QueueDashboardError> {
    let client = pool.get().await?;

    let row = client
        .query_opt(
            "SELECT\
                message_id,\
                email_subject,\
                status,\
                recommended_method,\
                final_method,\
                locked_by,\
                processing_started_at,\
                last_error,\
                decision_reason,\
                requires_manual_review,\
                retry_count,\
                email_received_at,\
                updated_at,\
                partial_fields,\
                priority,\
                llm_latency_ms,\
                manual_review_reason,\
                created_at,\
                completed_at\
            FROM ses.extraction_queue\
            WHERE message_id = $1",
            &[&message_id],
        )
        .await?;

    Ok(row.map(|r| map_job_detail(&r)))
}
