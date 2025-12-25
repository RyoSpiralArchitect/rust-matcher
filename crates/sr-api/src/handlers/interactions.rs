use axum::{Json, extract::State};
use sr_common::api::interaction_event::{InteractionEventRequest, InteractionEventResponse};
use sr_common::db::insert_interaction_event;

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

pub async fn submit_interaction_event(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(payload): Json<InteractionEventRequest>,
) -> Result<Json<InteractionEventResponse>, ApiError> {
    let response = insert_interaction_event(&state.pool, &auth.subject, &payload).await?;
    Ok(Json(response))
}
