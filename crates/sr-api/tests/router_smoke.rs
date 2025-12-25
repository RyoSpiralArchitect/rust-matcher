use axum::{body::Body, http::Request, http::StatusCode};
use tower::ServiceExt;

#[tokio::test]
async fn livez_healthy_and_queue_requires_auth() {
    let state = sr_api::test_state("test-key");
    let app = sr_api::create_router(state);

    let livez_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/livez")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(livez_response.status(), StatusCode::OK);

    let unauthorized = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/queue/jobs")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);

    let interaction_events = app
        .oneshot(
            Request::builder()
                .uri("/api/interactions/events")
                .method("POST")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(interaction_events.status(), StatusCode::UNAUTHORIZED);
}
