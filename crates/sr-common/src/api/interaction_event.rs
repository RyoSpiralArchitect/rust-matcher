use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEventType {
    ViewedCandidateDetail,
    CopiedTemplate,
    ClickedContact,
    Shortlisted,
}

impl InteractionEventType {
    pub fn as_str(&self) -> &'static str {
        match self {
            InteractionEventType::ViewedCandidateDetail => "viewed_candidate_detail",
            InteractionEventType::CopiedTemplate => "copied_template",
            InteractionEventType::ClickedContact => "clicked_contact",
            InteractionEventType::Shortlisted => "shortlisted",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEventSource {
    Gui,
    Crm,
    Api,
    Import,
}

impl InteractionEventSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            InteractionEventSource::Gui => "gui",
            InteractionEventSource::Crm => "crm",
            InteractionEventSource::Api => "api",
            InteractionEventSource::Import => "import",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEventStatus {
    Created,
    Updated,
}

impl InteractionEventStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            InteractionEventStatus::Created => "created",
            InteractionEventStatus::Updated => "updated",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionEventRequest {
    pub interaction_id: i64,
    pub event_type: InteractionEventType,
    pub idempotency_key: String,
    #[serde(default)]
    pub source: Option<InteractionEventSource>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InteractionEventResponse {
    pub id: i64,
    pub status: InteractionEventStatus,
}
