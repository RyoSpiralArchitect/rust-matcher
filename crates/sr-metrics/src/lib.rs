use std::env;
use std::sync::OnceLock;

use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing::{info, warn};

static PROMETHEUS_HANDLE: OnceLock<PrometheusHandle> = OnceLock::new();

/// Initialize a Prometheus exporter listening on `0.0.0.0:<port>`.
///
/// The port is resolved from the provided environment variable name or the
/// supplied `default_port`. Returns a handle to the exporter if it was started.
pub fn init_metrics(port_env: &str, default_port: u16) -> Option<&'static PrometheusHandle> {
    let port = env::var(port_env)
        .ok()
        .and_then(|raw| raw.parse::<u16>().ok())
        .unwrap_or(default_port);

    if let Some(existing) = PROMETHEUS_HANDLE.get() {
        return Some(existing);
    }

    match PrometheusBuilder::new()
        .with_http_listener(([0, 0, 0, 0], port))
        .install_recorder()
    {
        Ok(handle) => {
            let _ = PROMETHEUS_HANDLE.set(handle);
            info!(metrics_port = port, "started prometheus exporter");
            PROMETHEUS_HANDLE.get()
        }
        Err(err) => {
            warn!(error = %err, metrics_port = port, "failed to start prometheus exporter");
            PROMETHEUS_HANDLE.get()
        }
    }
}
