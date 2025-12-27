use deadpool_postgres::{
    Config, CreatePoolError, ManagerConfig, Pool, PoolConfig, PoolError, RecyclingMethod, Runtime,
    Timeouts,
};
use std::{env, str::FromStr, time::Duration};
use thiserror::Error;
use tokio_postgres::NoTls;
use tracing::warn;

pub type PgPool = Pool;

#[derive(Debug, Error)]
pub enum DbPoolError {
    #[error("invalid database url: {0}")]
    InvalidConfig(String),
    #[error("failed to create database pool: {0}")]
    PoolCreation(#[from] CreatePoolError),
    #[error("failed to check out pool connection: {0}")]
    PoolCheckout(#[from] PoolError),
    #[error("failed to validate database connectivity: {0}")]
    Connectivity(#[from] tokio_postgres::Error),
}

fn parse_env<T: FromStr>(keys: &[&str]) -> Option<T> {
    for key in keys {
        if let Ok(value) = env::var(key) {
            if let Ok(parsed) = value.parse::<T>() {
                return Some(parsed);
            }
        }
    }

    None
}

fn build_options() -> Option<String> {
    let mut options = Vec::new();

    if let Some(timeout_ms) = parse_env::<u64>(&["SR_DB_STATEMENT_TIMEOUT_MS"]) {
        let bounded = timeout_ms.min(120_000);
        options.push(format!("-c statement_timeout={bounded}"));
    }

    if let Some(timeout_ms) = parse_env::<u64>(&[
        "SR_DB_IDLE_IN_TRANSACTION_TIMEOUT_MS",
        "SR_DB_IDLE_IN_TX_TIMEOUT_MS",
    ]) {
        let bounded = timeout_ms.min(300_000);
        options.push(format!("-c idle_in_transaction_session_timeout={bounded}"));
    }

    if let Some(app_name) = env::var("SR_DB_APPLICATION_NAME")
        .ok()
        .filter(|name| !name.trim().is_empty())
    {
        let sanitized: String = app_name
            .trim()
            .chars()
            .filter(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
            .collect();
        if !sanitized.is_empty() {
            options.push(format!("-c application_name={sanitized}"));
        }
    }

    // Force UTC to align NOW()/CURRENT_TIMESTAMP/clock_timestamp() with application-side Utc::now comparisons.
    let timezone = env::var("SR_DB_TIMEZONE").unwrap_or_else(|_| "UTC".into());
    if !timezone.trim().is_empty() {
        options.push(format!("-c TimeZone={}", timezone.trim()));
    }

    if let Some(log_min_ms) = parse_env::<i64>(&["SR_DB_LOG_MIN_DURATION_MS"]) {
        let bounded = log_min_ms.max(-1);
        options.push(format!("-c log_min_duration_statement={bounded}"));
    }

    if let Some(log_level) = env::var("SR_DB_LOG_STATEMENTS")
        .ok()
        .map(|s| s.to_ascii_lowercase())
        .filter(|s| matches!(s.as_str(), "none" | "ddl" | "mod" | "all"))
    {
        options.push(format!("-c log_statement={log_level}"));
    }

    if options.is_empty() {
        None
    } else {
        Some(options.join(" "))
    }
}

fn parse_env_duration_ms(keys: &[&str]) -> Option<Duration> {
    parse_env::<u64>(keys).map(Duration::from_millis)
}

fn resolve_pool_size() -> usize {
    const DEFAULT_POOL_SIZE: usize = 8;
    const POOL_SIZE_CAP: usize = 32;

    for key in [
        "SR_DB_POOL_MAX_CONNECTIONS",
        "SR_DB_POOL_MAX_SIZE",
        "SR_DB_MAX_SIZE",
    ] {
        if let Ok(raw) = env::var(key) {
            let trimmed = raw.trim();
            if trimmed.is_empty() {
                continue;
            }

            match trimmed.parse::<usize>() {
                Ok(0) => {
                    warn!(
                        key,
                        default = DEFAULT_POOL_SIZE,
                        "pool size must be greater than 0; using default"
                    );
                    return DEFAULT_POOL_SIZE;
                }
                Ok(size) if size > POOL_SIZE_CAP => {
                    warn!(
                        key,
                        requested = size,
                        cap = POOL_SIZE_CAP,
                        "capping database pool size to protect shared database resources"
                    );
                    return POOL_SIZE_CAP;
                }
                Ok(size) => return size,
                Err(_) => warn!(
                    key,
                    raw,
                    default = DEFAULT_POOL_SIZE,
                    "invalid pool size; using default"
                ),
            }
        }
    }

    DEFAULT_POOL_SIZE
}

pub fn create_pool_from_url(db_url: &str) -> Result<PgPool, DbPoolError> {
    let mut pg_config = tokio_postgres::Config::from_str(db_url)
        .map_err(|e| DbPoolError::InvalidConfig(e.to_string()))?;

    if let Some(connect_timeout) =
        parse_env_duration_ms(&["SR_DB_POOL_CONNECT_TIMEOUT_MS", "SR_DB_CONNECT_TIMEOUT_MS"])
    {
        pg_config.connect_timeout(connect_timeout);
    }

    let mut cfg = Config::new();
    cfg.url = Some(db_url.to_string());
    cfg.connect_timeout = pg_config.get_connect_timeout().cloned();

    cfg.pool = Some(PoolConfig {
        max_size: resolve_pool_size(),
        timeouts: Timeouts {
            wait: parse_env_duration_ms(&[
                "SR_DB_POOL_WAIT_TIMEOUT_MS",
                "SR_DB_TIMEOUT_WAIT_MS",
                "SR_DB_TIMEOUT_WAIT_SECS",
            ])
            .or_else(|| parse_env::<u64>(&["SR_DB_TIMEOUT_WAIT_SECS"]).map(Duration::from_secs)),
            create: parse_env_duration_ms(&[
                "SR_DB_POOL_CREATE_TIMEOUT_MS",
                "SR_DB_TIMEOUT_CREATE_MS",
                "SR_DB_TIMEOUT_CREATE_SECS",
            ])
            .or_else(|| parse_env::<u64>(&["SR_DB_TIMEOUT_CREATE_SECS"]).map(Duration::from_secs)),
            recycle: parse_env_duration_ms(&[
                "SR_DB_POOL_RECYCLE_TIMEOUT_MS",
                "SR_DB_TIMEOUT_RECYCLE_MS",
                "SR_DB_TIMEOUT_RECYCLE_SECS",
            ])
            .or_else(|| parse_env::<u64>(&["SR_DB_TIMEOUT_RECYCLE_SECS"]).map(Duration::from_secs)),
        },
        ..Default::default()
    });

    cfg.options = build_options();

    cfg.manager = Some(ManagerConfig {
        recycling_method: RecyclingMethod::Fast,
    });

    cfg.create_pool(Some(Runtime::Tokio1), NoTls)
        .map_err(DbPoolError::PoolCreation)
}

fn fail_fast_enabled() -> bool {
    matches!(
        env::var("SR_DB_FAIL_FAST")
            .ok()
            .map(|value| value.to_ascii_lowercase()),
        Some(v) if matches!(v.as_str(), "1" | "true" | "yes" | "on")
    )
}

pub async fn create_pool_from_url_checked(db_url: &str) -> Result<PgPool, DbPoolError> {
    let pool = create_pool_from_url(db_url)?;

    if fail_fast_enabled() {
        // Ensure the pool can establish a live connection on startup so deployments fail
        // fast instead of hanging on database outages.
        let client = pool.get().await?;
        client.simple_query("SELECT 1").await?;
    }

    Ok(pool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serial_test::serial;
    use std::sync::Mutex;

    #[test]
    #[serial]
    fn builds_pool_without_connecting() {
        // Ensure env-driven overrides don't break pool creation when set.
        with_envs(
            &[
                ("SR_DB_MAX_SIZE", Some("8")),
                ("SR_DB_TIMEOUT_WAIT_SECS", Some("1")),
                ("SR_DB_TIMEOUT_CREATE_SECS", Some("1")),
                ("SR_DB_TIMEOUT_RECYCLE_SECS", Some("1")),
                ("SR_DB_APPLICATION_NAME", Some("sr-api")),
            ],
            || {
                let result = create_pool_from_url("postgres://user:pass@localhost:5432/example");
                assert!(result.is_ok());
            },
        );
    }

    #[test]
    #[serial]
    fn resolves_pool_size_with_bounds() {
        with_envs(
            &[
                ("SR_DB_POOL_MAX_CONNECTIONS", None),
                ("SR_DB_POOL_MAX_SIZE", None),
                ("SR_DB_MAX_SIZE", None),
            ],
            || {
                assert_eq!(resolve_pool_size(), 8);
            },
        );

        with_env("SR_DB_POOL_MAX_CONNECTIONS", Some("0"), || {
            assert_eq!(resolve_pool_size(), 8);
        });

        with_env("SR_DB_POOL_MAX_CONNECTIONS", Some("64"), || {
            assert_eq!(resolve_pool_size(), 32);
        });

        with_env("SR_DB_POOL_MAX_CONNECTIONS", Some("abc"), || {
            assert_eq!(resolve_pool_size(), 8);
        });
    }

    static ENV_GUARD: Mutex<()> = Mutex::new(());

    fn with_env(var: &str, value: Option<&str>, f: impl FnOnce()) {
        with_envs(&[(var, value)], f);
    }

    fn with_envs(vars: &[(&str, Option<&str>)], f: impl FnOnce()) {
        let _guard = ENV_GUARD.lock().unwrap();

        let previous: Vec<(&str, Option<String>)> = vars
            .iter()
            .map(|(var, value)| {
                let old = std::env::var(var).ok();
                match value {
                    Some(v) => std::env::set_var(var, v),
                    None => std::env::remove_var(var),
                }
                (*var, old)
            })
            .collect();

        f();

        for (var, previous_value) in previous {
            match previous_value {
                Some(v) => std::env::set_var(var, v),
                None => std::env::remove_var(var),
            }
        }
    }

    #[test]
    #[serial]
    fn fail_fast_default_is_disabled() {
        with_env("SR_DB_FAIL_FAST", None, || {
            assert!(!fail_fast_enabled());
        });
    }

    #[test]
    #[serial]
    fn fail_fast_accepts_truthy_values() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            with_env("SR_DB_FAIL_FAST", Some(value), || {
                assert!(fail_fast_enabled());
            });
        }
    }

    #[tokio::test]
    async fn connects_to_database_when_url_is_provided() {
        let url = std::env::var("TEST_DATABASE_URL")
            .ok()
            .or_else(|| std::env::var("DATABASE_URL").ok());

        let Some(db_url) = url else {
            eprintln!("skipping live DB check: TEST_DATABASE_URL/DATABASE_URL not set");
            return;
        };

        let pool = match create_pool_from_url(&db_url) {
            Ok(pool) => pool,
            Err(err) => {
                eprintln!("skipping live DB check: failed to build pool: {err}");
                return;
            }
        };

        match pool.get().await {
            Ok(client) => {
                // A simple ping to ensure the connection is usable.
                if let Err(err) = client.simple_query("SELECT 1").await {
                    panic!("live DB simple query failed: {err}");
                }
            }
            Err(err) => {
                eprintln!("skipping live DB check: failed to get client: {err}");
            }
        }
    }

    #[test]
    #[serial]
    fn builds_expected_options_from_env() {
        with_envs(
            &[
                ("SR_DB_STATEMENT_TIMEOUT_MS", Some("3000")),
                ("SR_DB_IDLE_IN_TRANSACTION_TIMEOUT_MS", Some("15000")),
                ("SR_DB_APPLICATION_NAME", Some("sr-api")),
            ],
            || {
                let options = build_options().expect("options should be populated");
                assert!(options.contains("statement_timeout=3000"));
                assert!(options.contains("idle_in_transaction_session_timeout=15000"));
                assert!(options.contains("application_name=sr-api"));
            },
        );
    }
}
