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
}

pub fn create_pool_from_url(db_url: &str) -> Result<PgPool, DbPoolError> {
    let _ = tokio_postgres::Config::from_str(db_url)
        .map_err(|e| DbPoolError::InvalidConfig(e.to_string()))?;

    let mut cfg = Config::new();
    cfg.url = Some(db_url.to_string());

    cfg.pool = Some(PoolConfig {
        max_size: env::var("SR_DB_MAX_SIZE")
            .ok()
            .and_then(|value| value.parse().ok())
            .unwrap_or(16),
        timeouts: Timeouts {
            wait: Some(Duration::from_secs(
                env::var("SR_DB_TIMEOUT_WAIT_SECS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(5),
            )),
            create: Some(Duration::from_secs(
                env::var("SR_DB_TIMEOUT_CREATE_SECS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(5),
            )),
            recycle: Some(Duration::from_secs(
                env::var("SR_DB_TIMEOUT_RECYCLE_SECS")
                    .ok()
                    .and_then(|value| value.parse().ok())
                    .unwrap_or(5),
            )),
        },
        ..Default::default()
    });

    if let Ok(statement_timeout_ms) = env::var("SR_DB_STATEMENT_TIMEOUT_MS") {
        if let Ok(timeout_ms) = statement_timeout_ms.parse::<u64>() {
            cfg.options = Some(format!("-c statement_timeout={timeout_ms}"));
        }
    }

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
        let _ = pool.get().await?;
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
        }
        let result = create_pool_from_url("postgres://user:pass@localhost:5432/example");
        assert!(result.is_ok());
    }

    fn with_env(var: &str, value: Option<&str>, f: impl FnOnce()) {
        use std::sync::Mutex;
        static ENV_GUARD: Mutex<()> = Mutex::new(());
        let _guard = ENV_GUARD.lock().unwrap();

        let previous = std::env::var(var).ok();
        match value {
            Some(v) => unsafe { std::env::set_var(var, v) },
            None => unsafe { std::env::remove_var(var) },
        }

        f();

        match previous {
            Some(v) => unsafe { std::env::set_var(var, v) },
            None => unsafe { std::env::remove_var(var) },
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
}
