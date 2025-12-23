use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    body::Body,
    http::Method,
    http::Request,
    http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderName, HeaderValue},
    routing::{get, post},
};
use clap::Parser;
use dotenvy::dotenv;
use sr_common::api::match_response::MatchConfig;
use sr_common::db::PgPool;
use sr_common::db::create_pool_from_url_checked;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;

mod auth;
mod error;
mod handlers;

use auth::{AuthConfig, AuthMode, JwtAlgorithm};
use error::ApiError;
use handlers::{candidates, feedback, health, queue};

#[derive(Debug, Clone, Parser)]
#[command(name = "sr-api", about = "HTTP API for sr-match GUI integration")]
struct Cli {
    /// PostgreSQL connection string
    #[arg(long, env = "DATABASE_URL")]
    database_url: String,

    /// Server port
    #[arg(long, env = "PORT", default_value_t = 3001)]
    port: u16,

    /// API key for X-API-Key authentication
    #[arg(long, env = "SR_API_KEY")]
    api_key: Option<String>,

    /// Authentication mode: api_key | jwt
    #[arg(long, env = "AUTH_MODE", default_value = "api_key", value_enum)]
    auth_mode: AuthMode,

    /// JWT secret for AUTH_MODE=jwt
    #[arg(long, env = "JWT_SECRET")]
    jwt_secret: Option<String>,

    /// Public key for AUTH_MODE=jwt when using an asymmetric algorithm
    #[arg(long, env = "JWT_PUBLIC_KEY")]
    jwt_public_key: Option<String>,

    /// JWT algorithm (default aligns with NextAuth default HS512)
    #[arg(long, env = "JWT_ALGORITHM", default_value = "hs512", value_enum)]
    jwt_algorithm: JwtAlgorithm,

    /// Comma separated list of allowed CORS origins
    #[arg(long, env = "SR_CORS_ORIGINS", default_value = "http://localhost:3000")]
    cors_origins: String,

    /// Allow returning source_text previews on job detail
    #[arg(long, env = "SR_API_ALLOW_SOURCE_TEXT", default_value = "false")]
    allow_source_text: bool,

    /// Statement timeout (ms) applied while fetching job detail includes
    #[arg(
        long,
        env = "SR_API_JOB_DETAIL_STATEMENT_TIMEOUT_MS",
        default_value_t = 5000
    )]
    job_detail_statement_timeout_ms: i32,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    database_url: String,
    port: u16,
    cors_origins: Vec<String>,
    auth: AuthConfig,
    pub allow_source_text: bool,
    pub job_detail_statement_timeout_ms: i32,
}

impl AppConfig {
    fn from_cli(cli: Cli) -> Result<Self, ApiError> {
        let cors_origins = cli
            .cors_origins
            .split(',')
            .map(|origin| origin.trim().to_string())
            .filter(|origin| !origin.is_empty())
            .collect::<Vec<_>>();

        let auth = AuthConfig {
            mode: cli.auth_mode,
            api_key: cli.api_key,
            jwt_secret: cli.jwt_secret,
            jwt_public_key: cli.jwt_public_key,
            jwt_algorithm: cli.jwt_algorithm,
        };

        match auth.mode {
            AuthMode::ApiKey if auth.api_key.is_none() => {
                return Err(ApiError::BadRequest(
                    "SR_API_KEY is required when AUTH_MODE=api_key".into(),
                ));
            }
            AuthMode::Jwt => match auth.jwt_algorithm.key_kind() {
                auth::JwtKeyKind::Secret if auth.jwt_secret.is_none() => {
                    return Err(ApiError::BadRequest(
                        "JWT_SECRET is required when AUTH_MODE=jwt with symmetric algorithms"
                            .into(),
                    ));
                }
                auth::JwtKeyKind::Secret => {}
                _ if auth.jwt_public_key.is_none() => {
                    return Err(ApiError::BadRequest(
                        "JWT_PUBLIC_KEY is required when AUTH_MODE=jwt with asymmetric algorithms"
                            .into(),
                    ));
                }
                _ => {}
            },
            _ => {}
        }

        if cli.job_detail_statement_timeout_ms <= 0 {
            return Err(ApiError::BadRequest(
                "SR_API_JOB_DETAIL_STATEMENT_TIMEOUT_MS must be positive".into(),
            ));
        }

        Ok(Self {
            database_url: cli.database_url,
            port: cli.port,
            cors_origins,
            auth,
            allow_source_text: cli.allow_source_text,
            job_detail_statement_timeout_ms: cli.job_detail_statement_timeout_ms,
        })
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: AppConfig,
    pub match_config: MatchConfig,
}

pub type SharedState = Arc<AppState>;

impl axum::extract::FromRef<SharedState> for AuthConfig {
    fn from_ref(input: &SharedState) -> AuthConfig {
        input.config.auth.clone()
    }
}

fn cors_layer(origins: &[String]) -> CorsLayer {
    let allowed = origins
        .iter()
        .filter_map(|origin| origin.parse::<HeaderValue>().ok())
        .collect::<Vec<_>>();

    CorsLayer::new()
        .allow_origin(allowed)
        .allow_methods([Method::GET, Method::POST, Method::OPTIONS])
        .allow_headers([
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-api-key"),
        ])
}

fn create_router(state: SharedState) -> Router {
    let cors = cors_layer(&state.config.cors_origins);

    let request_id_header = HeaderName::from_static("x-request-id");
    let trace_header = request_id_header.clone();

    let trace = TraceLayer::new_for_http().make_span_with(move |request: &Request<Body>| {
        let request_id = request
            .headers()
            .get(&trace_header)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("");

        tracing::info_span!(
            "http_request",
            method = %request.method(),
            uri = %request.uri(),
            request_id = %request_id,
            status = tracing::field::Empty,
        )
    });

    let api_routes = Router::new()
        .route("/queue/dashboard", get(queue::dashboard))
        .route("/queue/jobs", get(queue::list_jobs))
        .route("/queue/jobs/:id", get(queue::get_job))
        .route("/queue/retry/:id", post(queue::retry_job))
        .route(
            "/projects/:project_id/candidates",
            get(candidates::list_candidates),
        )
        .route("/feedback", post(feedback::submit_feedback));

    Router::new()
        .route("/health", get(health::health_check))
        .nest("/api", api_routes)
        .layer(trace)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(
            request_id_header,
            MakeRequestUuid::default(),
        ))
        .layer(cors)
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{http::{Request, StatusCode}, routing::get};
    use tower::ServiceExt;

    #[tokio::test]
    async fn sets_request_id_when_missing() {
        let app = Router::new()
            .route("/", get(|| async { "ok" }))
            .layer(TraceLayer::new_for_http())
            .layer(PropagateRequestIdLayer::new(HeaderName::from_static(
                "x-request-id",
            )))
            .layer(SetRequestIdLayer::new(
                HeaderName::from_static("x-request-id"),
                MakeRequestUuid::default(),
            ));

        let response = app
            .oneshot(Request::builder().uri("/").body(Body::empty()).unwrap())
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert!(response.headers().contains_key("x-request-id"));
    }
}

async fn run() -> Result<(), ApiError> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = AppConfig::from_cli(cli)?;
    let pool = create_pool_from_url_checked(&config.database_url)
        .await
        .map_err(|err| ApiError::Database(format!("failed to create pool: {err}")))?;

    let state = Arc::new(AppState {
        pool,
        config: config.clone(),
        match_config: MatchConfig::from_env(),
    });

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let app = create_router(state);

    info!(%addr, auth_mode = ?config.auth.mode, "sr-api listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    let service = app.into_make_service_with_connect_info::<SocketAddr>();

    axum::serve(listener, service)
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    Ok(())
}

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        tracing::error!(error = %err, "sr-api failed");
        std::process::exit(1);
    }
}
