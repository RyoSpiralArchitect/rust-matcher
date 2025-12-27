use tracing::instrument;

use crate::api::models::queue::{QueueDashboard, StatusCounts};
use crate::db::util::TimedClientExt;
use crate::db::PgPool;

db_error!(QueueDashboardError {});

#[instrument(skip(pool))]
pub async fn fetch_dashboard(pool: &PgPool) -> Result<QueueDashboard, QueueDashboardError> {
    let client = pool.get().await?;

    let row = client
        .timed_query_one(
            "SELECT \
                COUNT(*) FILTER (WHERE status = 'pending') AS pending, \
                COUNT(*) FILTER (WHERE status = 'processing') AS processing, \
                COUNT(*) FILTER (WHERE status = 'completed') AS completed, \
                COUNT(*) FILTER (WHERE requires_manual_review) AS manual_review_count, \
                COUNT(*) FILTER (WHERE last_error IS NOT NULL) AS error_count, \
                COUNT(*) FILTER ( \
                    WHERE status = 'processing' \
                      AND processing_started_at <= timezone('utc', NOW()) - INTERVAL '10 minutes' \
                ) AS stale_processing_count, \
                COALESCE(MAX(updated_at), timezone('utc', NOW())) AS updated_at \
            FROM ses.extraction_queue",
            &[],
            "fetch_queue_dashboard",
        )
        .await?;

    Ok(QueueDashboard {
        status_counts: StatusCounts {
            pending: row.get::<_, i64>("pending"),
            processing: row.get::<_, i64>("processing"),
            completed: row.get::<_, i64>("completed"),
        },
        manual_review_count: row.get::<_, i64>("manual_review_count"),
        error_count: row.get::<_, i64>("error_count"),
        stale_processing_count: row.get::<_, i64>("stale_processing_count"),
        updated_at: row.get::<_, chrono::DateTime<chrono::Utc>>("updated_at"),
    })
}
