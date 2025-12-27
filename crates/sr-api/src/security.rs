use chrono::{Duration, Utc};

#[derive(Debug, Clone)]
pub struct SecurityTxtConfig {
    pub contact: String,
    pub expires: String,
    pub policy: Option<String>,
    pub acknowledgments: Option<String>,
    pub encryption: Option<String>,
    pub preferred_languages: Vec<String>,
    pub canonical: Option<String>,
    pub hiring: Option<String>,
}

impl SecurityTxtConfig {
    pub fn new(
        contact: String,
        expires: String,
        policy: Option<String>,
        acknowledgments: Option<String>,
        encryption: Option<String>,
        preferred_languages: Vec<String>,
        canonical: Option<String>,
        hiring: Option<String>,
    ) -> Self {
        Self {
            contact,
            expires,
            policy,
            acknowledgments,
            encryption,
            preferred_languages,
            canonical,
            hiring,
        }
    }

    pub fn with_defaults(contact: String, preferred_languages: Vec<String>) -> Self {
        let expires =
            (Utc::now() + Duration::days(180)).to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
        Self {
            contact,
            expires,
            policy: None,
            acknowledgments: None,
            encryption: None,
            preferred_languages,
            canonical: None,
            hiring: None,
        }
    }
}
