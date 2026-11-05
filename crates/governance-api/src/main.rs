pub mod auth;

use axum::{routing::get, Json, Router};
use governance_core::HealthStatus;
use sqlx::postgres::PgPoolOptions;
use std::{env, net::SocketAddr};

async fn live() -> Json<HealthStatus> {
    Json(HealthStatus { service: "governance-api".into(), ready: true })
}

async fn ready() -> Json<HealthStatus> {
    Json(HealthStatus { service: "governance-api".into(), ready: true })
}

#[tokio::main]
async fn main() {
    let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("APP_PORT").unwrap_or_else(|_| "8080".into());
    let address: SocketAddr = format!("{host}:{port}").parse().expect("valid APP_HOST/APP_PORT");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new().max_connections(10).acquire_timeout(std::time::Duration::from_secs(5)).connect(&database_url).await.expect("connect to PostgreSQL");
    sqlx::migrate!("../../migrations").run(&pool).await.expect("run database migrations");
    let app = Router::new().route("/health/live", get(live)).route("/health/ready", get(ready));
    let listener = tokio::net::TcpListener::bind(address).await.expect("bind API listener");
    println!("governance-api listening on {address}");
    axum::serve(listener, app).await.expect("serve API");
}
