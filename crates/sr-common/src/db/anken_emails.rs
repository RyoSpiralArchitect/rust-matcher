use chrono::{DateTime, Utc};
use deadpool_postgres::PoolError;
use tokio_postgres::Error as PgError;

use crate::db::PgPool;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingEmail {
    pub message_id: String,
    pub subject: String,
    pub body_text: String,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, thiserror::Error)]
pub enum PendingEmailError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
}

/// Fetch the raw body text for a specific message_id from `ses.anken_emails`.
pub async fn fetch_email_body(
    pool: &PgPool,
    message_id: &str,
) -> Result<Option<String>, PendingEmailError> {
    let client = pool.get().await?;
    let stmt = client
        .prepare("SELECT body_text FROM ses.anken_emails WHERE message_id = $1")
        .await?;

    let row = client.query_opt(&stmt, &[&message_id]).await?;
    Ok(row.and_then(|r| r.get::<_, Option<String>>("body_text")))
}

/// Fetch pending emails that have not been enqueued into the extraction queue yet.
///
/// This mirrors the reference query in MVP_PLAN.md: select up to `limit` rows from
/// `ses.anken_emails` that are missing from `ses.extraction_queue`, ordered by
/// newest first.
pub async fn fetch_pending_emails(
    pool: &PgPool,
    limit: i64,
) -> Result<Vec<PendingEmail>, PendingEmailError> {
    let client = pool.get().await?;

    let stmt = client
        .prepare(
            "SELECT ae.message_id, ae.subject, ae.body_text, ae.created_at
             FROM ses.anken_emails ae
             LEFT JOIN ses.extraction_queue eq ON ae.message_id = eq.message_id
             WHERE eq.id IS NULL
             ORDER BY ae.created_at DESC
             LIMIT $1",
        )
        .await?;

    let rows = client.query(&stmt, &[&limit]).await?;

    Ok(rows
        .into_iter()
        .filter_map(|row| {
            let body_text: Option<String> = row.get("body_text");
            body_text.map(|body| PendingEmail {
                message_id: row.get("message_id"),
                subject: row.get("subject"),
                body_text: body,
                created_at: row.get::<_, DateTime<Utc>>("created_at"),
            })
        })
        .collect())
}
