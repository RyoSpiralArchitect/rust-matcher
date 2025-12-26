use chrono::{DateTime, Utc};
use deadpool_postgres::GenericClient;
use tokio_postgres::Error as PgError;
use tracing::instrument;

use crate::api::feedback_request::{FeedbackRequest, NgReasonCategory};
use crate::api::feedback_response::{FeedbackResponse, FeedbackStatus};
use crate::db::{db_error, validated_actor, PgPool};

db_error!(FeedbackStorageError {
    #[error("interaction not found: {0}")]
    InteractionNotFound(i64),
    #[error("feedback actor is missing")]
    MissingActor,
});

struct InteractionContext {
    interaction_id: i64,
    match_result_id: Option<i64>,
    match_run_id: Option<String>,
    engine_version: Option<String>,
    config_version: Option<String>,
    project_id: i64,
    talent_id: i64,
}

fn map_ng_reason(value: &Option<NgReasonCategory>) -> Option<&str> {
    value.as_ref().map(|reason| reason.as_str())
}

/// Recompute the canonical outcome / feedback_at on interaction_logs based on
/// non-revoked feedback_events using the priority rules from
/// TwoTower_SalesFBアルゴ概観.md.
async fn recompute_interaction_outcome(
    client: &impl GenericClient,
    interaction_id: i64,
) -> Result<(), PgError> {
    let row = client
        .query_opt(
            "SELECT feedback_type, created_at
             FROM ses.feedback_events
             WHERE interaction_id = $1 AND is_revoked = false
             ORDER BY
               CASE feedback_type
                 WHEN 'accepted' THEN 1
                 WHEN 'rejected' THEN 2
                 WHEN 'interview_scheduled' THEN 3
                 WHEN 'review_ok' THEN 4
                 WHEN 'review_ng' THEN 4
                 WHEN 'thumbs_up' THEN 5
                 WHEN 'thumbs_down' THEN 5
                 WHEN 'no_response' THEN 6
                 ELSE 100
               END ASC,
               created_at DESC
             LIMIT 1",
            &[&interaction_id],
        )
        .await?;

    let (outcome, feedback_at): (Option<String>, Option<DateTime<Utc>>) = match row {
        Some(row) => (Some(row.get("feedback_type")), Some(row.get("created_at"))),
        None => (None, None),
    };

    client
        .execute(
            "UPDATE ses.interaction_logs
             SET outcome = $1, feedback_at = $2
             WHERE id = $3",
            &[&outcome, &feedback_at, &interaction_id],
        )
        .await?;

    Ok(())
}

#[instrument(skip(client, interaction_id))]
async fn fetch_interaction_context(
    client: &impl GenericClient,
    interaction_id: i64,
) -> Result<InteractionContext, FeedbackStorageError> {
    let row = client
        .query_opt(
            "SELECT\
                id,\
                match_result_id,\
                match_run_id,\
                engine_version,\
                config_version,\
                project_id,\
                talent_id\
            FROM ses.interaction_logs\
            WHERE id = $1",
            &[&interaction_id],
        )
        .await?
        .ok_or(FeedbackStorageError::InteractionNotFound(interaction_id))?;

    Ok(InteractionContext {
        interaction_id: row.get::<_, i64>("id"),
        match_result_id: row.get("match_result_id"),
        match_run_id: row.get("match_run_id"),
        engine_version: row.get("engine_version"),
        config_version: row.get("config_version"),
        project_id: row.get::<_, i64>("project_id"),
        talent_id: row.get::<_, i64>("talent_id"),
    })
}

#[instrument(skip(pool, actor, request))]
pub async fn insert_feedback_event(
    pool: &PgPool,
    actor: &str,
    request: &FeedbackRequest,
) -> Result<FeedbackResponse, FeedbackStorageError> {
    let actor = validated_actor(actor).ok_or(FeedbackStorageError::MissingActor)?;

    let mut client = pool.get().await?;
    let tx = client.transaction().await?;
    let interaction = fetch_interaction_context(&tx, request.interaction_id).await?;

    let stmt = tx
        .prepare_cached(
            "INSERT INTO ses.feedback_events (\
                interaction_id,\
                match_result_id,\
                match_run_id,\
                engine_version,\
                config_version,\
                project_id,\
                talent_id,\
                feedback_type,\
                ng_reason_category,\
                comment,\
                actor,\
                source\
            ) VALUES (\
                $1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12\
            )\
            ON CONFLICT (interaction_id, feedback_type, actor) DO NOTHING\
            RETURNING id",
        )
        .await?;

    let row = tx
        .query_opt(
            &stmt,
            &[
                &interaction.interaction_id,
                &interaction.match_result_id,
                &interaction.match_run_id,
                &interaction.engine_version,
                &interaction.config_version,
                &interaction.project_id,
                &interaction.talent_id,
                &request.feedback_type.as_str(),
                &map_ng_reason(&request.ng_reason_category),
                &request.comment,
                &actor,
                &request.source.as_str(),
            ],
        )
        .await?;

    let response = match row {
        Some(row) => FeedbackResponse {
            id: Some(row.get("id")),
            status: FeedbackStatus::Created,
        },
        None => FeedbackResponse {
            id: None,
            status: FeedbackStatus::AlreadyExists,
        },
    };

    // Keep interaction_logs outcome in sync with the highest-priority, latest feedback.
    recompute_interaction_outcome(&tx, interaction.interaction_id).await?;

    tx.commit().await?;

    Ok(response)
}
