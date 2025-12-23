use sr_api::run;

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

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        tracing::error!(error = %err, "sr-api failed");
        std::process::exit(1);
    }
}
