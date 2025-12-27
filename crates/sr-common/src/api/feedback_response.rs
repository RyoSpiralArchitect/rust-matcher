use serde::{Deserialize, Serialize};
use strum::AsRefStr;

use crate::api::feedback_request::FeedbackType;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackStatus {
    Created,
    AlreadyExists,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackResponse {
    pub id: Option<i64>,
    pub status: FeedbackStatus,
    pub feedback_type: FeedbackType,
    pub interaction_id: i64,
    pub match_result_id: Option<i64>,
    pub match_run_id: Option<String>,
    pub project_id: i64,
    pub talent_id: i64,
}
