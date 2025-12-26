use serde::Deserialize;

use crate::Project;

/// HTTP APIからのマッチリクエスト
#[derive(Debug, Clone, Deserialize)]
pub struct MatchRequest {
    pub project: Project,
    #[serde(default)]
    pub talent_ids: Option<Vec<i64>>,
    #[serde(default)]
    pub include_softko: bool,
    #[serde(default)]
    pub limit: Option<usize>,
}
