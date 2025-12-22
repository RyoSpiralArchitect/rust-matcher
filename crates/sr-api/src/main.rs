use std::net::SocketAddr;
use std::sync::Arc;

use axum::{
    Router,
    http::Method,
    http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderName, HeaderValue},
    routing::{get, post},
};
use clap::Parser;
use dotenvy::dotenv;
use sr_common::api::match_response::MatchConfig;
use sr_common::db::PgPool;
use sr_common::db::create_pool_from_url;
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::info;

mod auth;
mod error;
mod handlers;

use auth::{AuthConfig, AuthMode};
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

    /// Comma separated list of allowed CORS origins
    #[arg(long, env = "SR_CORS_ORIGINS", default_value = "http://localhost:3000")]
    cors_origins: String,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    database_url: String,
    port: u16,
    cors_origins: Vec<String>,
    auth: AuthConfig,
}

impl From<Cli> for AppConfig {
    fn from(cli: Cli) -> Self {
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
        };

        Self {
            database_url: cli.database_url,
            port: cli.port,
            cors_origins,
            auth,
        }
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

    let api_routes = Router::new()
        .route("/queue/dashboard", get(queue::dashboard))
        .route(
            "/projects/:project_id/candidates",
            get(candidates::list_candidates),
        )
        .route("/feedback", post(feedback::submit_feedback));

    Router::new()
        .route("/health", get(health::health_check))
        .nest("/api", api_routes)
        .layer(TraceLayer::new_for_http())
        .layer(cors)
        .with_state(state)
}

async fn run() -> Result<(), ApiError> {
    dotenv().ok();
    tracing_subscriber::fmt::init();

    let cli = Cli::parse();
    let config = AppConfig::from(cli);
    let pool = create_pool_from_url(&config.database_url)
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
