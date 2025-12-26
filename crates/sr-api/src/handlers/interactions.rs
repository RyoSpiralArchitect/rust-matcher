use axum::{Json, extract::State};
use sr_common::api::interaction_event::{InteractionEventRequest, InteractionEventResponse};
use sr_common::db::insert_interaction_event;

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

const MAX_META_LEN: usize = 4000;

pub async fn submit_interaction_event(
    State(state): State<SharedState>,
    auth: AuthUser,
    Json(payload): Json<InteractionEventRequest>,
) -> Result<Json<InteractionEventResponse>, ApiError> {
    if let Some(meta) = &payload.meta {
        let meta_len = serde_json::to_string(meta).map(|s| s.len()).unwrap_or(0);
        if meta_len > MAX_META_LEN {
            return Err(ApiError::BadRequest(format!(
                "meta payload too large (max {} bytes)",
                MAX_META_LEN
            )));
        }
    }

    let response = insert_interaction_event(&state.pool, &auth.subject, &payload).await?;
    Ok(Json(response))
}
