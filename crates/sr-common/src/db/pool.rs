use deadpool_postgres::{
    Config, CreatePoolError, ManagerConfig, Pool, PoolConfig, RecyclingMethod, Runtime, Timeouts,
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
}
