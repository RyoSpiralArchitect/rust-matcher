use axum::{
    Json,
    extract::{Path, Query, State},
};
use serde::Deserialize;
use sr_common::api::match_response::MatchResponse;
use sr_common::db::fetch_candidates_for_project;

use crate::SharedState;
use crate::auth::AuthUser;
use crate::error::ApiError;

#[derive(Debug, Deserialize, Default)]
pub struct CandidateQuery {
    #[serde(default)]
    pub include_softko: bool,
    #[serde(default = "default_limit")]
    pub limit: u32,
    #[serde(default)]
    pub offset: u32,
}

const fn default_limit() -> u32 {
    50
}

pub async fn list_candidates(
    State(state): State<SharedState>,
    Path(project_id): Path<i64>,
    Query(query): Query<CandidateQuery>,
    _auth: AuthUser,
) -> Result<Json<Vec<MatchResponse>>, ApiError> {
    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.min(10_000);

    let candidates = fetch_candidates_for_project(
        &state.pool,
        project_id,
        query.include_softko,
        limit,
        offset,
        &state.match_config,
    )
    .await?;

    Ok(Json(candidates))
}
