use axum::{Json, extract::State};
use sr_common::api::feedback_request::FeedbackRequest;
use sr_common::api::feedback_response::FeedbackResponse;
use sr_common::db::insert_feedback_event;

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

pub async fn submit_feedback(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(payload): Json<FeedbackRequest>,
) -> Result<Json<FeedbackResponse>, ApiError> {
    let response = insert_feedback_event(&state.pool, &auth.subject, &payload).await?;
    Ok(Json(response))
}
