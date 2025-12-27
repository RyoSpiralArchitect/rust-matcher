use std::env;
use std::net::{IpAddr, SocketAddr};
use std::num::NonZeroU32;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    body::Body,
    extract::connect_info::ConnectInfo,
    extract::DefaultBodyLimit,
    extract::State,
    http::header::{HeaderName, HeaderValue, AUTHORIZATION, CONTENT_TYPE},
    http::Method,
    http::Request,
    http::StatusCode,
    middleware,
    middleware::Next,
    response::Response,
    routing::{get, post},
    Router,
};
use chrono::{Duration as ChronoDuration, Utc};
use clap::Parser;
use dotenvy::dotenv;
use governor::{
    clock::{Clock, DefaultClock},
    middleware::{StateInformationMiddleware, StateSnapshot},
    state::keyed::DashMapStateStore,
    Quota, RateLimiter,
};
use metrics::{counter, gauge, histogram};
use sr_common::api::match_response::MatchConfig;
use sr_common::db::create_pool_from_url_checked;
use sr_common::db::{run_migrations, PgPool};
use sr_metrics::init_metrics;
use tokio::time::Duration as TokioDuration;
use tower_http::{
    cors::CorsLayer,
    request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer},
    trace::TraceLayer,
};
use tracing::{error, info};

pub mod auth;
pub mod error;
pub mod handlers;
pub mod security;

use auth::{AuthConfig, AuthMode, JwtAlgorithm};
use error::{ApiError, RateLimitMeta};
use handlers::{
    candidates, conversion, feedback, health, interactions, matches, queue,
    security as security_handler,
};
use security::SecurityTxtConfig;
use sr_common::logging::{init_tracing_subscriber, install_tracing_panic_hook};

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

    /// Use cookie-based JWT auth (__Host-sr-token) instead of Authorization header
    #[arg(long, env = "SR_API_USE_COOKIE_AUTH", default_value_t = false)]
    use_cookie_auth: bool,

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

    /// Contact for /.well-known/security.txt (mailto:, tel:, or https://)
    #[arg(
        long,
        env = "SR_SECURITY_CONTACT",
        default_value = "mailto:security@example.com"
    )]
    security_contact: String,

    /// Days until security.txt expires (must be positive)
    #[arg(long, env = "SR_SECURITY_EXPIRES_DAYS", default_value_t = 180)]
    security_expires_days: i64,

    /// Preferred languages for security.txt (comma separated)
    #[arg(long, env = "SR_SECURITY_PREFERRED_LANGS", default_value = "en")]
    security_preferred_langs: String,

    /// Optional policy URL for security.txt
    #[arg(long, env = "SR_SECURITY_POLICY")]
    security_policy: Option<String>,

    /// Optional acknowledgments URL for security.txt
    #[arg(long, env = "SR_SECURITY_ACKNOWLEDGMENTS")]
    security_acknowledgments: Option<String>,

    /// Optional encryption key URL for security.txt
    #[arg(long, env = "SR_SECURITY_ENCRYPTION")]
    security_encryption: Option<String>,

    /// Optional canonical URL for security.txt
    #[arg(long, env = "SR_SECURITY_CANONICAL")]
    security_canonical: Option<String>,

    /// Optional hiring URL for security.txt
    #[arg(long, env = "SR_SECURITY_HIRING")]
    security_hiring: Option<String>,
}

#[derive(Debug, Clone)]
pub struct AppConfig {
    pub database_url: String,
    pub port: u16,
    pub cors_origins: Vec<String>,
    pub auth: AuthConfig,
    pub allow_source_text: bool,
    pub job_detail_statement_timeout_ms: i32,
    pub security_txt: SecurityTxtConfig,
}

type IpRateLimiter =
    RateLimiter<IpAddr, DashMapStateStore<IpAddr>, DefaultClock, StateInformationMiddleware>;

#[derive(Clone)]
pub struct RateLimits {
    global: Arc<IpRateLimiter>,
    health: Arc<IpRateLimiter>,
    match_requests: Arc<IpRateLimiter>,
    retry: Arc<IpRateLimiter>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RateLimitConfig {
    pub global_per_sec: u64,
    pub global_burst: u32,
    pub retry_per_sec: u64,
    pub retry_burst: u32,
    pub health_per_sec: u64,
    pub health_burst: u32,
    pub match_per_sec: u64,
    pub match_burst: u32,
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
        let global_per_sec = Self::parse_env_u64(&["SR_RATE_LIMIT_GLOBAL_PER_SEC"]).unwrap_or(20);
        let global_burst = Self::parse_env_u32(&["SR_RATE_LIMIT_GLOBAL_BURST"]).unwrap_or(40);
        let match_per_sec =
            Self::parse_env_u64(&["SR_RATE_LIMIT_MATCH_PER_SEC"]).unwrap_or(global_per_sec);
        let match_burst =
            Self::parse_env_u32(&["SR_RATE_LIMIT_MATCH_BURST"]).unwrap_or(global_burst);
        let health_per_sec = Self::parse_env_u64(&["SR_RATE_LIMIT_HEALTH_PER_SEC"])
            .unwrap_or(global_per_sec.saturating_mul(5));
        let health_burst = Self::parse_env_u32(&["SR_RATE_LIMIT_HEALTH_BURST"])
            .unwrap_or(global_burst.saturating_mul(5));
        Self {
            global_per_sec,
            global_burst,
            retry_per_sec: Self::parse_env_u64(&["SR_RATE_LIMIT_RETRY_PER_SEC"]).unwrap_or(1),
            retry_burst: Self::parse_env_u32(&["SR_RATE_LIMIT_RETRY_BURST"]).unwrap_or(3),
            health_per_sec,
            health_burst,
            match_per_sec,
            match_burst,
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
            use_cookie_auth: cli.use_cookie_auth,
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

        if cli.security_contact.trim().is_empty() {
            return Err(ApiError::BadRequest(
                "SR_SECURITY_CONTACT cannot be empty".into(),
            ));
        }

        if !is_valid_security_contact(&cli.security_contact) {
            return Err(ApiError::BadRequest(
                "SR_SECURITY_CONTACT must start with mailto:, https://, http://, or tel:".into(),
            ));
        }

        if cli.security_expires_days <= 0 {
            return Err(ApiError::BadRequest(
                "SR_SECURITY_EXPIRES_DAYS must be positive".into(),
            ));
        }

        let preferred_languages = cli
            .security_preferred_langs
            .split(',')
            .map(|lang| lang.trim().to_string())
            .filter(|lang| !lang.is_empty())
            .collect::<Vec<_>>();

        let expires = (Utc::now() + ChronoDuration::days(cli.security_expires_days))
            .to_rfc3339_opts(chrono::SecondsFormat::Secs, true);

        let security_txt = SecurityTxtConfig {
            contact: cli.security_contact.clone(),
            expires,
            policy: cli.security_policy.clone(),
            acknowledgments: cli.security_acknowledgments.clone(),
            encryption: cli.security_encryption.clone(),
            preferred_languages,
            canonical: cli.security_canonical.clone(),
            hiring: cli.security_hiring.clone(),
        };

        Ok(Self {
            database_url: cli.database_url,
            port: cli.port,
            cors_origins,
            auth,
            allow_source_text: cli.allow_source_text,
            job_detail_statement_timeout_ms: cli.job_detail_statement_timeout_ms,
            security_txt,
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
            security_txt: SecurityTxtConfig::with_defaults(
                "mailto:security@example.com".into(),
                vec!["en".into()],
            ),
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

fn is_valid_security_contact(contact: &str) -> bool {
    let contact = contact.trim().to_ascii_lowercase();
    contact.starts_with("mailto:")
        || contact.starts_with("https://")
        || contact.starts_with("http://")
        || contact.starts_with("tel:")
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

    Arc::new(RateLimiter::keyed(quota).with_middleware::<StateInformationMiddleware>())
}

pub fn default_rate_limits() -> RateLimits {
    let cfg = RateLimitConfig::from_env();
    RateLimits {
        global: build_ip_limiter(cfg.global_per_sec, cfg.global_burst),
        health: build_ip_limiter(cfg.health_per_sec, cfg.health_burst),
        match_requests: build_ip_limiter(cfg.match_per_sec, cfg.match_burst),
        retry: build_ip_limiter(cfg.retry_per_sec, cfg.retry_burst),
    }
}

fn request_ip<B>(req: &Request<B>) -> Option<IpAddr> {
    req.extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|info| info.0.ip())
}

fn rate_limit_meta_from_snapshot(snapshot: StateSnapshot) -> RateLimitMeta {
    let quota = snapshot.quota();
    let limit = quota.burst_size().get();
    let remaining = snapshot.remaining_burst_capacity().min(limit);
    let used = limit.saturating_sub(remaining);
    RateLimitMeta {
        limit,
        remaining,
        reset_after: quota.replenish_interval().saturating_mul(used.max(1)),
        retry_after: None,
    }
}

fn rate_limit_meta_from_rejection(
    rejection: governor::NotUntil<<DefaultClock as Clock>::Instant>,
) -> RateLimitMeta {
    let wait_time = rejection.wait_time_from(DefaultClock::default().now());
    let quota = rejection.quota();
    RateLimitMeta {
        limit: quota.burst_size().get(),
        remaining: 0,
        reset_after: wait_time,
        retry_after: Some(wait_time),
    }
}

fn enforce_rate_limit(
    limiter: &IpRateLimiter,
    ip: Option<IpAddr>,
) -> Result<Option<RateLimitMeta>, ApiError> {
    if let Some(client_ip) = ip {
        match limiter.check_key(&client_ip) {
            Ok(snapshot) => Ok(Some(rate_limit_meta_from_snapshot(snapshot))),
            Err(rejection) => Err(ApiError::TooManyRequests {
                message: "rate limit exceeded".into(),
                rate_limit: Some(rate_limit_meta_from_rejection(rejection)),
            }),
        }
    } else {
        Ok(None)
    }
}

fn rate_limiter_for_path<'a>(rate_limits: &'a RateLimits, path: &str) -> &'a IpRateLimiter {
    if matches!(path, "/health" | "/readyz" | "/livez") {
        return rate_limits.health.as_ref();
    }

    if path.ends_with("/match") {
        return rate_limits.match_requests.as_ref();
    }

    rate_limits.global.as_ref()
}

async fn per_endpoint_rate_limit(
    State(state): State<SharedState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let limiter = rate_limiter_for_path(&state.rate_limits, req.uri().path());
    let meta = enforce_rate_limit(limiter, request_ip(&req))?;
    let mut response = next.run(req).await;
    if let Some(meta) = meta {
        meta.apply_headers(response.headers_mut());
    }
    Ok(response)
}

async fn retry_rate_limit(
    State(state): State<SharedState>,
    req: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let meta = enforce_rate_limit(&state.rate_limits.retry, request_ip(&req))?;
    let mut response = next.run(req).await;
    if let Some(meta) = meta {
        meta.apply_headers(response.headers_mut());
    }
    Ok(response)
}

async fn attach_request_id_context(req: Request<Body>, next: Next) -> Result<Response, ApiError> {
    let request_id = req
        .headers()
        .get("x-request-id")
        .and_then(|value| value.to_str().ok())
        .map(|value| value.to_string());

    Ok(error::with_request_id(request_id, next.run(req)).await)
}

async fn record_http_metrics(req: Request<Body>, next: Next) -> Result<Response, ApiError> {
    let method = req.method().to_string();
    let path = req.uri().path().to_string();
    let start = Instant::now();
    let response = next.run(req).await;

    let latency_ms = start.elapsed().as_secs_f64() * 1000.0;
    let status = response.status().as_u16().to_string();

    histogram!(
        "http_request_latency_ms",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status.clone(),
    )
    .record(latency_ms);

    counter!(
        "http_requests_total",
        "method" => method,
        "path" => path,
        "status" => status,
    )
    .increment(1);

    Ok(response)
}

async fn apply_security_headers(req: Request<Body>, next: Next) -> Result<Response, ApiError> {
    let mut response = next.run(req).await;

    response
        .headers_mut()
        .entry(HeaderName::from_static("x-content-type-options"))
        .or_insert_with(|| HeaderValue::from_static("nosniff"));
    response
        .headers_mut()
        .entry(HeaderName::from_static("x-frame-options"))
        .or_insert_with(|| HeaderValue::from_static("DENY"));

    Ok(response)
}

async fn log_body_limit_rejections(req: Request<Body>, next: Next) -> Result<Response, ApiError> {
    let path = req.uri().path().to_string();
    let client_ip = request_ip(&req)
        .map(|ip| ip.to_string())
        .unwrap_or_else(|| "".to_string());

    let response = next.run(req).await;

    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        error!(
            status = %response.status(),
            request_id = error::current_request_id().as_deref().unwrap_or(""),
            client_ip = client_ip.as_str(),
            path = %path,
            "request_body_too_large"
        );
    }

    Ok(response)
}

fn spawn_pool_metrics(pool: PgPool) {
    tokio::spawn(async move {
        let mut ticker = tokio::time::interval(TokioDuration::from_secs(10));
        loop {
            ticker.tick().await;
            let status = pool.status();
            gauge!("db_pool_size", "pool" => "primary").set(status.size as f64);
            gauge!("db_pool_available", "pool" => "primary").set(status.available as f64);
            gauge!("db_pool_waiting", "pool" => "primary").set(status.waiting as f64);
        }
    });
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
        .route("/match", post(matches::run_match))
        .route("/matches/:match_id", get(matches::get_match))
        .route(
            "/projects/:project_id/candidates",
            get(candidates::list_candidates),
        )
        .route("/feedback", post(feedback::submit_feedback))
        .route(
            "/feedback/history/:interaction_id",
            get(feedback::get_feedback_history),
        )
        .route(
            "/interactions/events",
            post(interactions::submit_interaction_event),
        )
        .route("/conversions", post(conversion::submit_conversion));

    Router::new()
        .route(
            "/.well-known/security.txt",
            get(security_handler::security_txt),
        )
        .route("/health", get(health::readyz))
        .route("/livez", get(health::livez))
        .route("/readyz", get(health::readyz))
        .nest("/api/v1", api_routes.clone())
        .nest("/api", api_routes)
        .layer(middleware::from_fn_with_state(
            state.clone(),
            per_endpoint_rate_limit,
        ))
        .layer(middleware::from_fn(attach_request_id_context))
        .layer(middleware::from_fn(record_http_metrics))
        .layer(DefaultBodyLimit::max(256 * 1024))
        .layer(middleware::from_fn(log_body_limit_rejections))
        .layer(middleware::from_fn(apply_security_headers))
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
        use_cookie_auth: false,
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
        body::Body,
        extract::connect_info::ConnectInfo,
        http::{Request, StatusCode},
        routing::get,
    };
    use http_body_util::BodyExt;
    use serial_test::serial;
    use std::{net::SocketAddr, sync::Mutex};
    use tower::ServiceExt;

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn with_envs(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let _guard = ENV_GUARD.lock().unwrap();

        let previous: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(var, value)| {
                let old = env::var(var).ok();
                match value {
                    Some(v) => env::set_var(var, v),
                    None => env::remove_var(var),
                }
                (*var, old)
            })
            .collect();

        let outcome = std::panic::catch_unwind(std::panic::AssertUnwindSafe(f));

        for (var, previous_value) in previous {
            match previous_value {
                Some(v) => env::set_var(var, v),
                None => env::remove_var(var),
            }
        }

        if let Err(panic) = outcome {
            std::panic::resume_unwind(panic);
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
    #[serial]
    fn rate_limit_config_respects_env_overrides() {
        with_envs(
            &[
                ("SR_RATE_LIMIT_GLOBAL_PER_SEC", Some("10")),
                ("SR_RATE_LIMIT_GLOBAL_BURST", Some("25")),
                ("SR_RATE_LIMIT_RETRY_PER_SEC", Some("2")),
                ("SR_RATE_LIMIT_RETRY_BURST", Some("5")),
                ("SR_RATE_LIMIT_HEALTH_PER_SEC", Some("50")),
                ("SR_RATE_LIMIT_HEALTH_BURST", Some("75")),
                ("SR_RATE_LIMIT_MATCH_PER_SEC", Some("7")),
                ("SR_RATE_LIMIT_MATCH_BURST", Some("11")),
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
                        health_per_sec: 50,
                        health_burst: 75,
                        match_per_sec: 7,
                        match_burst: 11,
                    }
                );
            },
        );
    }

    #[test]
    fn attaches_rate_limit_headers_on_successful_requests() {
        with_envs(
            &[
                ("SR_RATE_LIMIT_GLOBAL_PER_SEC", None),
                ("SR_RATE_LIMIT_GLOBAL_BURST", None),
                ("SR_RATE_LIMIT_RETRY_PER_SEC", None),
                ("SR_RATE_LIMIT_RETRY_BURST", None),
                ("SR_RATE_LIMIT_HEALTH_PER_SEC", None),
                ("SR_RATE_LIMIT_HEALTH_BURST", None),
                ("SR_RATE_LIMIT_MATCH_PER_SEC", None),
                ("SR_RATE_LIMIT_MATCH_BURST", None),
            ],
            || {
                tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .unwrap()
                    .block_on(async {
                        let state = test_state("test-key");
                        let app = Router::new()
                            .route("/", get(|| async { "ok" }))
                            .layer(middleware::from_fn_with_state(
                                state.clone(),
                                per_endpoint_rate_limit,
                            ))
                            .with_state(state);

                        let req = Request::builder()
                            .uri("/")
                            .extension(ConnectInfo(SocketAddr::from(([127, 0, 0, 1], 12345))))
                            .body(Body::empty())
                            .unwrap();

                        let response = app.oneshot(req).await.expect("request should succeed");

                        let limit = response.headers().get("ratelimit-limit");
                        let remaining = response.headers().get("ratelimit-remaining");
                        let reset = response.headers().get("ratelimit-reset");

                        let limit = limit
                            .and_then(|h| h.to_str().ok())
                            .and_then(|v| v.parse::<u32>().ok());
                        let remaining = remaining
                            .and_then(|h| h.to_str().ok())
                            .and_then(|v| v.parse::<u32>().ok());
                        let reset = reset
                            .and_then(|h| h.to_str().ok())
                            .and_then(|v| v.parse::<u64>().ok());

                        assert!(limit.is_some());
                        assert!(remaining.is_some());
                        assert!(reset.is_some());

                        let limit = limit.unwrap();
                        let remaining = remaining.unwrap();
                        assert!(remaining <= limit);
                        assert!(limit > 0);
                        assert!(reset.unwrap() > 0);
                    });
            },
        );
    }

    #[test]
    fn chooses_rate_limiter_by_path() {
        let limits = default_rate_limits();

        assert!(std::ptr::eq(
            rate_limiter_for_path(&limits, "/health"),
            limits.health.as_ref()
        ));
        assert!(std::ptr::eq(
            rate_limiter_for_path(&limits, "/api/v1/match"),
            limits.match_requests.as_ref()
        ));
        assert!(std::ptr::eq(
            rate_limiter_for_path(&limits, "/api/v1/queue/jobs"),
            limits.global.as_ref()
        ));
    }

    fn base_cli() -> Cli {
        Cli {
            database_url: "postgres://user:pass@localhost:5432/example".into(),
            port: 3001,
            api_key: Some("test-key".into()),
            auth_mode: AuthMode::ApiKey,
            jwt_secret: None,
            jwt_public_key: None,
            jwt_algorithm: JwtAlgorithm::Hs512,
            use_cookie_auth: false,
            cors_origins: "http://localhost:3000".into(),
            allow_source_text: false,
            job_detail_statement_timeout_ms: 5000,
            security_contact: "mailto:security@example.com".into(),
            security_expires_days: 180,
            security_preferred_langs: "en".into(),
            security_policy: None,
            security_acknowledgments: None,
            security_encryption: None,
            security_canonical: None,
            security_hiring: None,
        }
    }

    #[test]
    fn rejects_invalid_security_contact_and_expiry() {
        let mut cli = base_cli();
        cli.security_contact = "   ".into();
        let err = AppConfig::from_cli(cli).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));

        let mut cli = base_cli();
        cli.security_contact = "ftp://example.com".into();
        let err = AppConfig::from_cli(cli).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));

        let mut cli = base_cli();
        cli.security_expires_days = 0;
        let err = AppConfig::from_cli(cli).unwrap_err();
        assert!(matches!(err, ApiError::BadRequest(_)));
    }

    #[tokio::test]
    async fn applies_security_headers_globally() {
        let state = test_state("test-key");
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/livez")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response
                .headers()
                .get("x-content-type-options")
                .and_then(|v| v.to_str().ok()),
            Some("nosniff")
        );
        assert_eq!(
            response
                .headers()
                .get("x-frame-options")
                .and_then(|v| v.to_str().ok()),
            Some("DENY")
        );
    }

    #[tokio::test]
    async fn serves_security_txt_with_configured_metadata() {
        let state = test_state("test-key");
        let app = create_router(state);

        let response = app
            .oneshot(
                Request::builder()
                    .uri("/.well-known/security.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);

        let body = response.into_body().collect().await.unwrap().to_bytes();
        let body_str = String::from_utf8(body.to_vec()).unwrap();

        assert!(body_str.contains("Contact: mailto:security@example.com"));
        assert!(body_str.contains("Expires:"));
        assert!(body_str.contains("Preferred-Languages: en"));
    }
}

pub async fn run() -> Result<(), ApiError> {
    dotenv().ok();
    init_tracing_subscriber(env!("CARGO_PKG_NAME"));
    install_tracing_panic_hook(env!("CARGO_PKG_NAME"));
    init_metrics("SR_API_METRICS_PORT", 9899);

    let cli = Cli::parse();
    let config = AppConfig::from_cli(cli)?;
    let match_config = MatchConfig::from_env_checked().map_err(ApiError::BadRequest)?;
    let pool = create_pool_from_url_checked(&config.database_url)
        .await
        .map_err(|err| ApiError::database_error(format!("failed to create pool: {err}")))?;
    run_migrations(&pool)
        .await
        .map_err(|err| ApiError::database_error(format!("failed to run migrations: {err}")))?;

    let rate_limits = default_rate_limits();

    let state = Arc::new(AppState {
        pool,
        config: config.clone(),
        match_config,
        rate_limits,
        readiness: Arc::new(std::sync::atomic::AtomicBool::new(true)),
    });

    let addr: SocketAddr = ([0, 0, 0, 0], config.port).into();
    let app = create_router(state.clone());
    spawn_pool_metrics(state.pool.clone());

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
        use tokio::signal::unix::{signal, SignalKind};
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
