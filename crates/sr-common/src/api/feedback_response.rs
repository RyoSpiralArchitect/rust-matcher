use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackStatus {
    Created,
    AlreadyExists,
}

impl FeedbackStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedbackStatus::Created => "created",
            FeedbackStatus::AlreadyExists => "already_exists",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackResponse {
    pub id: Option<i64>,
    pub status: FeedbackStatus,
}
