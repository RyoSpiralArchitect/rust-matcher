use sr_api::run;

#[tokio::main]
async fn main() {
    if let Err(err) = run().await {
        tracing::error!(error = %err, "sr-api failed");
        std::process::exit(1);
    }
}
