use axum::{
    extract::{Path, Query, State},
    Json,
};
use serde::Deserialize;
use sr_common::api::match_response::MatchResponse;
use sr_common::db::fetch_candidates_for_project;

use crate::auth::AuthUser;
use crate::error::ApiError;
use crate::SharedState;

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

#[derive(Debug, serde::Serialize)]
pub struct CandidateListResponse {
    pub project_id: i64,
    pub candidates: Vec<MatchResponse>,
}

pub async fn list_candidates(
    State(state): State<SharedState>,
    Path(project_id): Path<i64>,
    Query(query): Query<CandidateQuery>,
    _auth: AuthUser,
) -> Result<Json<CandidateListResponse>, ApiError> {
    let limit = query.limit.clamp(1, 200);
    let offset = query.offset.min(10_000);

    let candidates = fetch_candidates_for_project(
        &state.pool,
        project_id,
        query.include_softko,
        limit,
        offset,
        None,
        &state.match_config,
    )
    .await?;

    Ok(Json(CandidateListResponse {
        project_id,
        candidates,
    }))
}
