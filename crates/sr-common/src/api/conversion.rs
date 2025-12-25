use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversionStage {
    Contacted,
    Entry,
    InterviewScheduled,
    Offer,
    ContractSigned,
    Lost,
}

impl ConversionStage {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConversionStage::Contacted => "contacted",
            ConversionStage::Entry => "entry",
            ConversionStage::InterviewScheduled => "interview_scheduled",
            ConversionStage::Offer => "offer",
            ConversionStage::ContractSigned => "contract_signed",
            ConversionStage::Lost => "lost",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConversionSource {
    Gui,
    Crm,
    Import,
}

impl ConversionSource {
    pub fn as_str(&self) -> &'static str {
        match self {
            ConversionSource::Gui => "gui",
            ConversionSource::Crm => "crm",
            ConversionSource::Import => "import",
        }
    }
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
