#![allow(async_fn_in_trait)]

use deadpool_postgres::GenericClient;
use serde_json::Value;
use std::{sync::OnceLock, time::Instant};
use tokio_postgres::{types::Json, ToStatement};
use tracing::warn;

/// Convert an optional JSON value into a Postgres-compatible wrapper.
pub fn normalize_json(value: &Option<Value>) -> Option<Json<&Value>> {
    value.as_ref().map(Json)
}

fn slow_query_threshold_ms() -> Option<u64> {
    static CACHE: OnceLock<Option<u64>> = OnceLock::new();

    *CACHE.get_or_init(|| {
        std::env::var("SR_DB_LOG_MIN_DURATION_MS")
            .ok()
            .and_then(|raw| raw.parse::<i64>().ok())
            .map(|v| v.max(0) as u64)
            .filter(|v| *v > 0)
    })
}

fn maybe_log_slow_query(label: &str, started_at: Instant) {
    if let Some(threshold_ms) = slow_query_threshold_ms() {
        let elapsed = started_at.elapsed();
        let elapsed_ms = elapsed.as_millis() as u64;
        if elapsed_ms >= threshold_ms {
            warn!(query = label, elapsed_ms, "slow_query_detected");
        }
    }
}

pub trait TimedClientExt: GenericClient {
    async fn timed_query<S>(
        &self,
        statement: &S,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<Vec<tokio_postgres::Row>, tokio_postgres::Error>
    where
        S: ToStatement + Sync + Send + ?Sized,
    {
        let started = Instant::now();
        let result = self.query(statement, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_query_cached(
        &self,
        statement: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<Vec<tokio_postgres::Row>, tokio_postgres::Error> {
        let started = Instant::now();
        let prepared = self.prepare_cached(statement).await?;
        let result = self.query(&prepared, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_query_opt<S>(
        &self,
        statement: &S,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<Option<tokio_postgres::Row>, tokio_postgres::Error>
    where
        S: ToStatement + Sync + Send + ?Sized,
    {
        let started = Instant::now();
        let result = self.query_opt(statement, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_query_opt_cached(
        &self,
        statement: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<Option<tokio_postgres::Row>, tokio_postgres::Error> {
        let started = Instant::now();
        let prepared = self.prepare_cached(statement).await?;
        let result = self.query_opt(&prepared, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_query_one<S>(
        &self,
        statement: &S,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<tokio_postgres::Row, tokio_postgres::Error>
    where
        S: ToStatement + Sync + Send + ?Sized,
    {
        let started = Instant::now();
        let result = self.query_one(statement, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_query_one_cached(
        &self,
        statement: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<tokio_postgres::Row, tokio_postgres::Error> {
        let started = Instant::now();
        let prepared = self.prepare_cached(statement).await?;
        let result = self.query_one(&prepared, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_execute<S>(
        &self,
        statement: &S,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<u64, tokio_postgres::Error>
    where
        S: ToStatement + Sync + Send + ?Sized,
    {
        let started = Instant::now();
        let result = self.execute(statement, params).await;
        maybe_log_slow_query(label, started);
        result
    }

    async fn timed_execute_cached(
        &self,
        statement: &str,
        params: &[&(dyn tokio_postgres::types::ToSql + Sync)],
        label: &str,
    ) -> Result<u64, tokio_postgres::Error> {
        let started = Instant::now();
        let prepared = self.prepare_cached(statement).await?;
        let result = self.execute(&prepared, params).await;
        maybe_log_slow_query(label, started);
        result
    }
}

impl<T: GenericClient + ?Sized> TimedClientExt for T {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_json_handles_options() {
        let none: Option<Value> = None;
        assert!(normalize_json(&none).is_none());

        let some = Some(serde_json::json!({"score": 0.9}));
        let normalized = normalize_json(&some);
        assert!(normalized.is_some());
    }
}
