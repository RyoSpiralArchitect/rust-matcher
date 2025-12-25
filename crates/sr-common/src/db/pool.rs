use deadpool_postgres::{
    Config, CreatePoolError, ManagerConfig, Pool, PoolConfig, PoolError, RecyclingMethod, Runtime,
    Timeouts,
};
use std::{env, str::FromStr, time::Duration};
use thiserror::Error;
use tokio_postgres::NoTls;

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
        options.push(format!("-c statement_timeout={timeout_ms}"));
    }

    if let Some(timeout_ms) = parse_env::<u64>(&[
        "SR_DB_IDLE_IN_TRANSACTION_TIMEOUT_MS",
        "SR_DB_IDLE_IN_TX_TIMEOUT_MS",
    ]) {
        options.push(format!(
            "-c idle_in_transaction_session_timeout={timeout_ms}"
        ));
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

    if options.is_empty() {
        None
    } else {
        Some(options.join(" "))
    }
}

/// Allow only safe characters for application_name to avoid injection via -c options.
fn sanitized_app_name() -> Option<String> {
    env::var("SR_DB_APPLICATION_NAME")
        .ok()
        .map(|name| name.trim().to_string())
        .filter(|name| !name.is_empty())
        .and_then(|name| {
            if name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.' | '/'))
            {
                Some(name)
            } else {
                None
            }
        })
}

pub fn create_pool_from_url(db_url: &str) -> Result<PgPool, DbPoolError> {
    let _ = tokio_postgres::Config::from_str(db_url)
        .map_err(|e| DbPoolError::InvalidConfig(e.to_string()))?;

    let mut cfg = Config::new();
    cfg.url = Some(db_url.to_string());

    cfg.pool = Some(PoolConfig {
        max_size: parse_env::<usize>(&["SR_DB_POOL_MAX_SIZE", "SR_DB_MAX_SIZE"]).unwrap_or(16),
        timeouts: Timeouts {
            wait: Some(Duration::from_secs(
                parse_env::<u64>(&["SR_DB_TIMEOUT_WAIT_SECS"]).unwrap_or(5),
            )),
            create: Some(Duration::from_secs(
                parse_env::<u64>(&["SR_DB_TIMEOUT_CREATE_SECS"]).unwrap_or(5),
            )),
            recycle: Some(Duration::from_secs(
                parse_env::<u64>(&["SR_DB_TIMEOUT_RECYCLE_SECS"]).unwrap_or(5),
            )),
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

    #[test]
    fn builds_pool_without_connecting() {
        // Ensure env-driven overrides don't break pool creation when set.
        unsafe {
            std::env::set_var("SR_DB_MAX_SIZE", "8");
            std::env::set_var("SR_DB_TIMEOUT_WAIT_SECS", "1");
            std::env::set_var("SR_DB_TIMEOUT_CREATE_SECS", "1");
            std::env::set_var("SR_DB_TIMEOUT_RECYCLE_SECS", "1");
            std::env::set_var("SR_DB_APPLICATION_NAME", "sr-api");
        }
        let result = create_pool_from_url("postgres://user:pass@localhost:5432/example");
        assert!(result.is_ok());
    }

    use std::sync::Mutex;
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
                    Some(v) => unsafe { std::env::set_var(var, v) },
                    None => unsafe { std::env::remove_var(var) },
                }
                (*var, old)
            })
            .collect();

        f();

        for (var, previous_value) in previous {
            match previous_value {
                Some(v) => unsafe { std::env::set_var(var, v) },
                None => unsafe { std::env::remove_var(var) },
            }
        }
    }

    #[test]
    fn fail_fast_default_is_disabled() {
        with_env("SR_DB_FAIL_FAST", None, || {
            assert!(!fail_fast_enabled());
        });
    }

    #[test]
    fn fail_fast_accepts_truthy_values() {
        for value in ["1", "true", "TRUE", "yes", "on"] {
            with_env("SR_DB_FAIL_FAST", Some(value), || {
                assert!(fail_fast_enabled());
            });
        }
    }

    #[test]
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
