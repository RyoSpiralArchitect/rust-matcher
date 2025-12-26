use axum::{body::Body, http::Request, http::StatusCode};
use std::sync::atomic::Ordering;
use tower::ServiceExt;

#[tokio::test]
async fn readyz_returns_service_unavailable_when_not_ready() {
    let state = sr_api::test_state("test-key");
    state.readiness.store(false, Ordering::SeqCst);
    let app = sr_api::create_router(state);

    let response = app
        .oneshot(
            Request::builder()
                .uri("/readyz")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
}
