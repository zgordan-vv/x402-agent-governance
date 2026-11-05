#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    tracing::info!("governance-worker started; durable jobs are not enabled yet");
    tokio::signal::ctrl_c().await.expect("install ctrl-c handler");
    tracing::info!("governance-worker stopped");
}
