use deadpool_postgres::PoolError;
use tokio_postgres::Error as PgError;
use tracing::instrument;

use crate::api::feedback_request::{FeedbackRequest, NgReasonCategory};
use crate::api::feedback_response::{FeedbackResponse, FeedbackStatus};
use crate::db::PgPool;

#[derive(Debug, thiserror::Error)]
pub enum FeedbackStorageError {
    #[error("failed to get postgres connection: {0}")]
    Pool(#[from] PoolError),
    #[error("postgres error: {0}")]
    Postgres(#[from] PgError),
    #[error("interaction not found: {0}")]
    InteractionNotFound(i64),
    #[error("feedback actor is missing" )]
    MissingActor,
}

struct InteractionContext {
    interaction_id: i64,
    match_result_id: Option<i32>,
    match_run_id: Option<String>,
    engine_version: Option<String>,
    config_version: Option<String>,
    project_id: i64,
    talent_id: i64,
}

fn map_ng_reason(value: &Option<NgReasonCategory>) -> Option<&str> {
    value.as_ref().map(|reason| reason.as_str())
}

#[instrument(skip(client, interaction_id))]
async fn fetch_interaction_context(
    client: &tokio_postgres::Client,
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
    let actor = actor.trim();
    if actor.is_empty() {
        return Err(FeedbackStorageError::MissingActor);
    }

    let client = pool.get().await?;
    let interaction = fetch_interaction_context(&client, request.interaction_id).await?;

    let stmt = client
        .prepare(
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

    let row = client
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

    Ok(match row {
        Some(row) => FeedbackResponse {
            id: Some(row.get("id")),
            status: FeedbackStatus::Created,
        },
        None => FeedbackResponse {
            id: None,
            status: FeedbackStatus::AlreadyExists,
        },
    })
}
