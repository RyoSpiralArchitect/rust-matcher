use serde::{Deserialize, Serialize};
use strum::AsRefStr;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStage {
    Contacted,
    Entry,
    InterviewScheduled,
    Offer,
    ContractSigned,
    Lost,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, AsRefStr)]
#[serde(rename_all = "snake_case")]
pub enum ConversionSource {
    Gui,
    Crm,
    Import,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionRequest {
    pub interaction_id: Option<i64>,
    pub talent_id: i64,
    pub project_id: i64,
    pub stage: ConversionStage,
    #[serde(default)]
    pub source: Option<ConversionSource>,
    pub meta: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionResponse {
    pub id: i64,
    pub status: String,
}
