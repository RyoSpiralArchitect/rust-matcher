use serde::{Deserialize, Serialize};
use strum::AsRefStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackType {
    ThumbsUp,
    ThumbsDown,
    ReviewOk,
    ReviewNg,
    ReviewPending,
    Accepted,
    Rejected,
    InterviewScheduled,
    NoResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum NgReasonCategory {
    Tanka,
    Skill,
    Availability,
    Location,
    Flow,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackSource {
    Gui,
    Crm,
    Api,
    Import,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackRequest {
    pub interaction_id: i64,
    pub feedback_type: FeedbackType,
    pub ng_reason_category: Option<NgReasonCategory>,
    pub comment: Option<String>,
    pub source: FeedbackSource,
}
