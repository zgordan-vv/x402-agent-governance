pub mod auth;
pub mod models;

use axum::{
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::get,
    Json, Router,
};
use governance_core::HealthStatus;
use models::{ErrorBody, ErrorEnvelope};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::{env, net::SocketAddr};
use tower_http::{limit::RequestBodyLimitLayer, timeout::TimeoutLayer};
use tracing_subscriber::EnvFilter;
use uuid::Uuid;

async fn request_id(mut request: axum::extract::Request, next: Next) -> Response {
    let id = request
        .headers()
        .get("x-request-id")
        .and_then(|v| v.to_str().ok())
        .filter(|v| !v.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
    request.extensions_mut().insert(id.clone());
    let mut response = next.run(request).await;
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response
}

async fn live() -> Json<HealthStatus> {
    Json(HealthStatus {
        service: "governance-api".into(),
        ready: true,
    })
}

async fn ready() -> Json<HealthStatus> {
    Json(HealthStatus {
        service: "governance-api".into(),
        ready: true,
    })
}

async fn openapi() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.0.3", "info": {"title": "x402 Agent Spend Governance API", "version": env!("CARGO_PKG_VERSION")},
        "paths": {"/health/live": {"get": {}}, "/health/ready": {"get": {}}, "/v1/me": {"get": {"security": [{"bearerAuth": []}]}}},
        "components": {"securitySchemes": {"bearerAuth": {"type": "http", "scheme": "bearer"}}}
    }))
}

async fn me(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    Ok(Json(
        serde_json::json!({ "workspace_id": principal.workspace_id, "agent_id": principal.agent_id, "environment_id": principal.environment_id, "roles": principal.roles.iter().map(|role| format!("{role:?}")).collect::<Vec<_>>() }),
    ))
}

async fn not_found(
    axum::extract::Extension(request_id): axum::extract::Extension<String>,
) -> (StatusCode, Json<ErrorEnvelope>) {
    (
        StatusCode::NOT_FOUND,
        Json(ErrorEnvelope {
            error: ErrorBody {
                code: "not_found",
                message: "route not found",
                request_id,
            },
        }),
    )
}

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();
    let host = env::var("APP_HOST").unwrap_or_else(|_| "0.0.0.0".into());
    let port = env::var("APP_PORT").unwrap_or_else(|_| "8080".into());
    let address: SocketAddr = format!("{host}:{port}")
        .parse()
        .expect("valid APP_HOST/APP_PORT");
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL is required");
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(&database_url)
        .await
        .expect("connect to PostgreSQL");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run database migrations");
    let app = Router::new()
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/openapi.json", get(openapi))
        .route("/v1/me", get(me))
        .with_state(pool.clone())
        .layer(RequestBodyLimitLayer::new(256 * 1024))
        .layer(TimeoutLayer::with_status_code(
            StatusCode::REQUEST_TIMEOUT,
            std::time::Duration::from_secs(10),
        ))
        .layer(middleware::from_fn(request_id))
        .fallback(not_found);
    let listener = tokio::net::TcpListener::bind(address)
        .await
        .expect("bind API listener");
    println!("governance-api listening on {address}");
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("serve API");
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("install ctrl-c handler");
    };
    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("install terminate handler")
            .recv()
            .await;
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
    tracing::info!("shutdown signal received");
}
