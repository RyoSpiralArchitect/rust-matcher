use deadpool_postgres::{Config, CreatePoolError, ManagerConfig, Pool, RecyclingMethod, Runtime};
use std::str::FromStr;
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
        let result = create_pool_from_url("postgres://user:pass@localhost:5432/example");
        assert!(result.is_ok());
    }
}
