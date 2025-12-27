use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, StatusCode},
    response::{IntoResponse, Response},
};

use crate::{security::SecurityTxtConfig, SharedState};

pub async fn security_txt(State(state): State<SharedState>) -> impl IntoResponse {
    let SecurityTxtConfig {
        contact,
        expires,
        policy,
        acknowledgments,
        encryption,
        preferred_languages,
        canonical,
        hiring,
    } = state.config.security_txt.clone();

    let mut body = format!("Contact: {contact}\nExpires: {expires}\n");

    if let Some(policy) = policy {
        body.push_str(&format!("Policy: {policy}\n"));
    }

    if let Some(acknowledgments) = acknowledgments {
        body.push_str(&format!("Acknowledgments: {acknowledgments}\n"));
    }

    if let Some(encryption) = encryption {
        body.push_str(&format!("Encryption: {encryption}\n"));
    }

    if !preferred_languages.is_empty() {
        body.push_str(&format!(
            "Preferred-Languages: {}\n",
            preferred_languages.join(",")
        ));
    }

    if let Some(canonical) = canonical {
        body.push_str(&format!("Canonical: {canonical}\n"));
    }

    if let Some(hiring) = hiring {
        body.push_str(&format!("Hiring: {hiring}\n"));
    }

    let mut response = Response::new(Body::from(body));
    *response.status_mut() = StatusCode::OK;
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("text/plain; charset=utf-8"),
    );

    response
}
