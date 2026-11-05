pub mod auth;

use axum::{http::StatusCode, routing::get, Json, Router};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer};
use governance_core::HealthStatus;
use sqlx::postgres::PgPoolOptions;
use std::{env, net::SocketAddr};
use tracing_subscriber::EnvFilter;

async fn live() -> Json<HealthStatus> {
    Json(HealthStatus { service: "governance-api".into(), ready: true })
}

async fn ready() -> Json<HealthStatus> {
    Json(HealthStatus { service: "governance-api".into(), ready: true })
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt().with_env_filter(EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"))).init();
    let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("APP_PORT").unwrap_or_else(|_| "8080".into());
    let address: SocketAddr = format!("{host}:{port}").parse().expect("valid APP_HOST/APP_PORT");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new().max_connections(10).acquire_timeout(std::time::Duration::from_secs(5)).connect(&database_url).await.expect("connect to PostgreSQL");
    sqlx::migrate!("../../migrations").run(&pool).await.expect("run database migrations");
    let app = Router::new().route("/health/live", get(live)).route("/health/ready", get(ready))
        .layer(RequestBodyLimitLayer::new(256 * 1024))
        .layer(TimeoutLayer::with_status_code(std::time::Duration::from_secs(10), StatusCode::REQUEST_TIMEOUT))
        .fallback(|| async { (StatusCode::NOT_FOUND, "not found") });
    let listener = tokio::net::TcpListener::bind(address).await.expect("bind API listener");
    println!("governance-api listening on {address}");
    axum::serve(listener, app).with_graceful_shutdown(shutdown_signal()).await.expect("serve API");
}

async fn shutdown_signal() {
    let ctrl_c = async { tokio::signal::ctrl_c().await.expect("install ctrl-c handler"); };
    #[cfg(unix)]
    let terminate = async { tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate()).expect("install terminate handler").recv().await; };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("shutdown signal received");
}
