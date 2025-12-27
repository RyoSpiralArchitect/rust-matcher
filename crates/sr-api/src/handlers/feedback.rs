use axum::{
    extract::{Path, State},
    Json,
};
use sr_common::api::feedback_request::FeedbackRequest;
use sr_common::api::feedback_response::FeedbackResponse;
use sr_common::api::models::queue::FeedbackEventRow;
use sr_common::db::{fetch_feedback_history, insert_feedback_event_tx};

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::SharedState;

const MAX_COMMENT_LEN: usize = 1_000;

pub async fn submit_feedback(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(payload): Json<FeedbackRequest>,
) -> Result<Json<FeedbackResponse>, ApiError> {
    if payload
        .comment
        .as_ref()
        .map(|c| c.len() > MAX_COMMENT_LEN)
        .unwrap_or(false)
    {
        return Err(ApiError::BadRequest(format!(
            "comment must be <= {} characters",
            MAX_COMMENT_LEN
        )));
    }

    let mut client = state
        .pool
        .get()
        .await
        .map_err(|err| ApiError::Database(err.to_string()))?;
    let tx = client
        .transaction()
        .await
        .map_err(|err| ApiError::Database(err.to_string()))?;
    let response = insert_feedback_event_tx(&tx, &auth.subject, &payload).await?;
    tx.commit()
        .await
        .map_err(|err| ApiError::Database(err.to_string()))?;
    Ok(Json(response))
}

pub async fn get_feedback_history(
    State(state): State<SharedState>,
    Path(interaction_id): Path<i64>,
    _auth: AuthUser,
) -> Result<Json<Vec<FeedbackEventRow>>, ApiError> {
    let events = fetch_feedback_history(&state.pool, interaction_id, 200).await?;
    Ok(Json(events))
}
