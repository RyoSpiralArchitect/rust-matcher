use axum::{
    extract::{Path, State},
    Json,
};
use sr_common::api::feedback_request::{FeedbackRequest, FeedbackType};
use sr_common::api::feedback_response::FeedbackResponse;
use sr_common::api::models::queue::FeedbackEventRow;
use sr_common::db::{fetch_feedback_history, insert_feedback_event_tx};

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::SharedState;

const MAX_COMMENT_LEN: usize = 1_000;

fn validate_feedback_payload(payload: &FeedbackRequest) -> Result<(), ApiError> {
    if payload.interaction_id <= 0 {
        return Err(ApiError::BadRequest(
            "interaction_id must be positive".into(),
        ));
    }

    let trimmed_comment = payload.comment.as_ref().map(|c| c.trim());

    if trimmed_comment.map(|c| c.is_empty()).unwrap_or(false) {
        return Err(ApiError::BadRequest("comment cannot be empty".into()));
    }

    if trimmed_comment
        .map(|c| c.len() > MAX_COMMENT_LEN)
        .unwrap_or(false)
    {
        return Err(ApiError::BadRequest(format!(
            "comment must be <= {} characters",
            MAX_COMMENT_LEN
        )));
    }

    if matches!(
        payload.feedback_type,
        FeedbackType::ThumbsDown | FeedbackType::ReviewNg | FeedbackType::Rejected
    ) && payload.ng_reason_category.is_none()
    {
        return Err(ApiError::BadRequest(
            "ng_reason_category is required for negative feedback".into(),
        ));
    }

    // Explicit validation makes sure payloads are not bypassing the client.
    match payload.feedback_type {
        FeedbackType::ThumbsUp
        | FeedbackType::ThumbsDown
        | FeedbackType::ReviewOk
        | FeedbackType::ReviewNg
        | FeedbackType::ReviewPending
        | FeedbackType::Accepted
        | FeedbackType::Rejected
        | FeedbackType::InterviewScheduled
        | FeedbackType::NoResponse => {}
    }

    Ok(())
}

pub async fn submit_feedback(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(payload): Json<FeedbackRequest>,
) -> Result<Json<FeedbackResponse>, ApiError> {
    validate_feedback_payload(&payload)?;

    let mut client = state
        .pool
        .get()
        .await
        .map_err(|err| ApiError::database_error(err))?;
    let tx = client
        .transaction()
        .await
        .map_err(|err| ApiError::database_error(err))?;
    let response = insert_feedback_event_tx(&tx, &auth.subject, &payload).await?;
    tx.commit()
        .await
        .map_err(|err| ApiError::database_error(err))?;
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
