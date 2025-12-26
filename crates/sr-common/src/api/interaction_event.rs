use serde::{Deserialize, Serialize};
use strum::AsRefStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEventType {
    ViewedCandidateDetail,
    CopiedTemplate,
    ClickedContact,
    Shortlisted,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEventSource {
    Gui,
    Crm,
    Api,
    Import,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum InteractionEventStatus {
    Created,
    Updated,
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
