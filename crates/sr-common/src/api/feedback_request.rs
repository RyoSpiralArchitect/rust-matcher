use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
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

impl FeedbackType {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedbackType::ThumbsUp => "thumbs_up",
            FeedbackType::ThumbsDown => "thumbs_down",
            FeedbackType::ReviewOk => "review_ok",
            FeedbackType::ReviewNg => "review_ng",
            FeedbackType::ReviewPending => "review_pending",
            FeedbackType::Accepted => "accepted",
            FeedbackType::Rejected => "rejected",
            FeedbackType::InterviewScheduled => "interview_scheduled",
            FeedbackType::NoResponse => "no_response",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NgReasonCategory {
    Tanka,
    Skill,
    Availability,
    Location,
    Flow,
    Other,
}

impl NgReasonCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            NgReasonCategory::Tanka => "tanka",
            NgReasonCategory::Skill => "skill",
            NgReasonCategory::Availability => "availability",
            NgReasonCategory::Location => "location",
            NgReasonCategory::Flow => "flow",
            NgReasonCategory::Other => "other",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackSource {
    Gui,
    Crm,
    Api,
    Import,
}

impl FeedbackSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            FeedbackSource::Gui => "gui",
            FeedbackSource::Crm => "crm",
            FeedbackSource::Api => "api",
            FeedbackSource::Import => "import",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FeedbackRequest {
    pub interaction_id: i64,
    pub feedback_type: FeedbackType,
    pub ng_reason_category: Option<NgReasonCategory>,
    pub comment: Option<String>,
    pub source: FeedbackSource,
}
