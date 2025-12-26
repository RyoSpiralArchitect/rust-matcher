use axum::{Json, extract::State};
use sr_common::api::feedback_request::FeedbackRequest;
use sr_common::api::feedback_response::FeedbackResponse;
use sr_common::db::insert_feedback_event;

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

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

    let response = insert_feedback_event(&state.pool, &auth.subject, &payload).await?;
    Ok(Json(response))
}
