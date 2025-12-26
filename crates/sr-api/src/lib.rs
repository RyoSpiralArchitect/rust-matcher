use std::env;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::Duration;

use axum::{
    Router,
    body::Body,
    extract::DefaultBodyLimit,
    extract::State,
    extract::connect_info::ConnectInfo,
    http::Method,
    http::Request,
    http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderName, HeaderValue},
    middleware,
    middleware::Next,
    response::Response,
    routing::{get, post},
};
use clap::Parser;
use dotenvy::dotenv;
use governor::{
    Quota, RateLimiter, clock::DefaultClock, middleware::NoOpMiddleware,
    state::keyed::DashMapStateStore,
};
use sr_common::api::match_response::MatchConfig;
use sr_common::db::create_pool_from_url_checked;
use sr_common::db::{PgPool, run_migrations};
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::info;

pub mod auth;
pub mod error;
pub mod handlers;

use auth::{AuthConfig, AuthMode, JwtAlgorithm};
use error::ApiError;
use handlers::{candidates, conversion, feedback, health, interactions, queue};
use sr_common::logging::install_tracing_panic_hook;

const SHUTDOWN_DRAIN_GRACE: std::time::Duration = std::time::Duration::from_millis(200);

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
    pub database_url: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub auth: AuthConfig,
    pub allow_source_text: bool,
    pub job_detail_statement_timeout_ms: i32,
}

type IpRateLimiter = RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock, NoOpMiddleware>;

#[derive(Clone)]
pub struct RateLimits {
    global: Arc<IpRateLimiter>,
    retry: Arc<IpRateLimiter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub global_per_sec: u64,
    pub global_burst: u32,
    pub retry_per_sec: u64,
    pub retry_burst: u32,
}

impl RateLimitConfig {
    fn parse_env_u64(vars: &[&str]) -> Option<u64> {
        vars.iter()
            .find_map(|name| env::var(name).ok())
            .and_then(|value| value.parse::<u64>().ok())
            .filter(|value| *value > 0)
    }

    fn parse_env_u32(vars: &[&str]) -> Option<u32> {
        vars.iter()
            .find_map(|name| env::var(name).ok())
            .and_then(|value| value.parse::<u32>().ok())
            .filter(|value| *value > 0)
    }

    fn from_env() -> Self {
        Self {
            global_per_sec: Self::parse_env_u64(&["SR_RATE_LIMIT_GLOBAL_PER_SEC"]).unwrap_or(20),
            global_burst: Self::parse_env_u32(&["SR_RATE_LIMIT_GLOBAL_BURST"]).unwrap_or(40),
            retry_per_sec: Self::parse_env_u64(&["SR_RATE_LIMIT_RETRY_PER_SEC"]).unwrap_or(1),
            retry_burst: Self::parse_env_u32(&["SR_RATE_LIMIT_RETRY_BURST"]).unwrap_or(3),
        }
    }
}

impl AppConfig {
    fn from_cli(cli: Cli) -> Result<Self, ApiError> {
        let cors_origins = cli
            .cors_origins
            .split(',')
            .map(|origin| origin.trim().to_string())
            .filter(|origin| !origin.is_empty())
            .collect::<Vec<_>>();

        if cors_origins.iter().any(|origin| origin == "*") {
            return Err(ApiError::BadRequest(
                "SR_CORS_ORIGINS must list explicit origins when credentials are enabled".into(),
            ));
        }

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

    pub fn for_tests(auth: AuthConfig) -> Self {
        Self {
            database_url: "postgres://user:pass@localhost:5432/example".into(),
            port: 3001,
            cors_origins: vec!["http://localhost:3000".into()],
            auth,
            allow_source_text: false,
            job_detail_statement_timeout_ms: 5000,
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: AppConfig,
    pub match_config: MatchConfig,
    pub(crate) rate_limits: RateLimits,
    pub readiness: Arc<std::sync::atomic::AtomicBool>,
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
        .allow_credentials(true)
}

fn build_ip_limiter(per_second: u64, burst_size: u32) -> Arc<IpRateLimiter> {
    let nanos_per_token = 1_000_000_000u64 / per_second.max(1);
    let quota = Quota::with_period(Duration::from_nanos(nanos_per_token.max(1)))
        .unwrap()
        .allow_burst(NonZeroU32::new(burst_size).unwrap());

    Arc::new(RateLimiter::keyed(quota))
}

pub fn default_rate_limits() -> RateLimits {
    let cfg = RateLimitConfig::from_env();
    RateLimits {
        global: build_ip_limiter(cfg.global_per_sec, cfg.global_burst),
        retry: build_ip_limiter(cfg.retry_per_sec, cfg.retry_burst),
    }
}

fn request_ip<B>(req: &Request<B>) -> Option<IpAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
}

fn enforce_rate_limit(limiter: &IpRateLimiter, ip: Option<IpAddr>) -> Result<(), ApiError> {
    if let Some(client_ip) = ip {
        if limiter.check_key(&client_ip).is_err() {
            return Err(ApiError::TooManyRequests("rate limit exceeded".into()));
        }
    }

    Ok(())
}

async fn global_rate_limit(
    State(state): State<SharedState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    enforce_rate_limit(&state.rate_limits.global, request_ip(&req))?;
    Ok(next.run(req).await)
}

async fn retry_rate_limit(
    State(state): State<SharedState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    enforce_rate_limit(&state.rate_limits.retry, request_ip(&req))?;
    Ok(next.run(req).await)
}

async fn attach_request_id_context(req: Request<Body>, next: Next) -> Result<Response, ApiError> {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    Ok(error::with_request_id(request_id, next.run(req)).await)
}

pub fn create_router(state: SharedState) -> Router {
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
        .route(
            "/queue/retry/:id",
            post(queue::retry_job).route_layer(middleware::from_fn_with_state(
                state.clone(),
                retry_rate_limit,
            )),
        )
        .route(
            "/projects/:project_id/candidates",
            get(candidates::list_candidates),
        )
        .route("/feedback", post(feedback::submit_feedback))
        .route(
            "/interactions/events",
            post(interactions::submit_interaction_event),
        )
        .route("/conversions", post(conversion::submit_conversion));

    Router::new()
        .route("/health", get(health::readyz))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .nest("/api", api_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            global_rate_limit,
        ))
        .layer(middleware::from_fn(attach_request_id_context))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(trace)
        .layer(PropagateRequestIdLayer::new(request_id_header.clone()))
        .layer(SetRequestIdLayer::new(
            request_id_header,
            MakeRequestUuid::default(),
        ))
        .layer(cors)
        .with_state(state)
}

pub fn test_state(api_key: &str) -> SharedState {
    let pool = sr_common::db::create_pool_from_url("postgres://user:pass@localhost:5432/example")
        .expect("pool should build without connecting");

    let auth = AuthConfig {
        mode: AuthMode::ApiKey,
        api_key: Some(api_key.to_string()),
        jwt_secret: None,
        jwt_public_key: None,
        jwt_algorithm: JwtAlgorithm::Hs256,
    };

    Arc::new(AppState {
        pool,
        config: AppConfig::for_tests(auth),
        match_config: MatchConfig::default(),
        rate_limits: default_rate_limits(),
        readiness: Arc::new(std::sync::atomic::AtomicBool::new(true)),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        http::{Request, StatusCode},
        routing::get,
    };
    use std::sync::Mutex;
    use tower::ServiceExt;

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn with_envs(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let _guard = ENV_GUARD.lock().unwrap();

        let previous: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(var, value)| {
                let old = env::var(var).ok();
                match value {
                    Some(v) => unsafe { env::set_var(var, v) },
                    None => unsafe { env::remove_var(var) },
                }
                (*var, old)
            })
            .collect();

        f();

        for (var, previous_value) in previous {
            match previous_value {
                Some(v) => unsafe { env::set_var(var, v) },
                None => unsafe { env::remove_var(var) },
            }
        }
    }

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

    #[test]
    fn rate_limit_config_respects_env_overrides() {
        with_envs(
            &[
                ("SR_RATE_LIMIT_GLOBAL_PER_SEC", Some("10")),
                ("SR_RATE_LIMIT_GLOBAL_BURST", Some("25")),
                ("SR_RATE_LIMIT_RETRY_PER_SEC", Some("2")),
                ("SR_RATE_LIMIT_RETRY_BURST", Some("5")),
            ],
            || {
                let cfg = RateLimitConfig::from_env();
                assert_eq!(
                    cfg,
                    RateLimitConfig {
                        global_per_sec: 10,
                        global_burst: 25,
                        retry_per_sec: 2,
                        retry_burst: 5,
                    }
                );
            },
        );
    }
}

pub async fn run() -> Result<(), ApiError> {
    dotenv().ok();
    tracing_subscriber::fmt::init();
    install_tracing_panic_hook(env!("CARGO_PKG_NAME"));

    let cli = Cli::parse();
    let config = AppConfig::from_cli(cli)?;
    let pool = create_pool_from_url_checked(&config.database_url)
        .await
        .map_err(|err| ApiError::Database(format!("failed to create pool: {err}")))?;
    run_migrations(&pool)
        .await
        .map_err(|err| ApiError::Database(format!("failed to run migrations: {err}")))?;

    let rate_limits = default_rate_limits();

    let state = Arc::new(AppState {
        pool,
        config: config.clone(),
        match_config: MatchConfig::from_env(),
        rate_limits,
        readiness: Arc::new(std::sync::atomic::AtomicBool::new(true)),
    });

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let app = create_router(state.clone());

    info!(%addr, auth_mode = ?config.auth.mode, "sr-api listening");

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    let service = app.into_make_service_with_connect_info::<SocketAddr>();

    axum::serve(listener, service)
        .with_graceful_shutdown(shutdown_signal(state.clone()))
        .await
        .map_err(|err| ApiError::Internal(err.to_string()))?;

    Ok(())
}

async fn shutdown_signal(state: SharedState) {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };

    #[cfg(unix)]
    let terminate = async {
        use tokio::signal::unix::{SignalKind, signal};
        if let Ok(mut sigterm) = signal(SignalKind::terminate()) {
            let _ = sigterm.recv().await;
        }
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }

    state
        .readiness
        .store(false, std::sync::atomic::Ordering::SeqCst);

    // Give load balancers a brief window to observe /readyz as not ready
    // before axum stops accepting new connections.
    tokio::time::sleep(SHUTDOWN_DRAIN_GRACE).await;
}
