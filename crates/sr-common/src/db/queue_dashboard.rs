use deadpool_postgres::PoolError;
use tokio_postgres::Error as PgError;
use tracing::instrument;

use crate::api::models::queue::{QueueDashboard, StatusCounts};
use crate::db::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum QueueDashboardError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
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
