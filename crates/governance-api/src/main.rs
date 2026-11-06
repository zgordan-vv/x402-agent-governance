pub mod auth;
pub mod evaluate;
pub mod models;
pub mod tokens;

use axum::{
    body::Body,
    extract::State,
    http::{HeaderMap, HeaderValue, StatusCode},
    middleware::{self, Next},
    response::Response,
    routing::{get, post},
    Json, Router,
};
use governance_core::HealthStatus;
use models::{
    AgentResponse, ApprovalDecisionRequest, ApprovalResponse, AuditEventResponse, ErrorBody,
    ErrorEnvelope, ExportRequest, FreezeWorkspaceRequest, PolicySummary, PrivacyProfile,
    SettlementRequest,
};
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
    if response.status().is_client_error() || response.status().is_server_error() {
        let status = response.status();
        let (code, message) = match status {
            StatusCode::BAD_REQUEST => ("bad_request", "request is invalid"),
            StatusCode::UNAUTHORIZED => ("unauthorized", "valid authentication is required"),
            StatusCode::FORBIDDEN => ("forbidden", "operation is not permitted"),
            StatusCode::NOT_FOUND => ("not_found", "resource was not found"),
            StatusCode::CONFLICT => ("conflict", "request conflicts with current state"),
            StatusCode::REQUEST_TIMEOUT => ("request_timeout", "request timed out"),
            StatusCode::SERVICE_UNAVAILABLE => ("service_unavailable", "service is not ready"),
            StatusCode::UNPROCESSABLE_ENTITY => (
                "unprocessable_entity",
                "request failed validation, including an expired quote",
            ),
            _ => ("internal_error", "request could not be completed"),
        };
        let body = serde_json::to_vec(&ErrorEnvelope {
            error: ErrorBody {
                code,
                message,
                request_id: id.clone(),
            },
        })
        .expect("error envelope serializes");
        *response.body_mut() = Body::from(body);
        response.headers_mut().insert(
            axum::http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
    }
    if let Ok(value) = HeaderValue::from_str(&id) {
        response.headers_mut().insert("x-request-id", value);
    }
    response.headers_mut().insert(
        "x-content-type-options",
        HeaderValue::from_static("nosniff"),
    );
    response
        .headers_mut()
        .insert("x-frame-options", HeaderValue::from_static("DENY"));
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response.headers_mut().insert(
        "content-security-policy",
        HeaderValue::from_static("default-src 'none'; frame-ancestors 'none'"),
    );
    if std::env::var("APP_ENV").as_deref() == Ok("production") {
        response.headers_mut().insert(
            "strict-transport-security",
            HeaderValue::from_static("max-age=31536000; includeSubDomains"),
        );
    }
    response
}

async fn live() -> Json<HealthStatus> {
    Json(HealthStatus {
        service: "governance-api".into(),
        ready: true,
    })
}

async fn index() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "service": "x402-agent-governance",
        "status": "running",
        "health": "/health/ready",
        "openapi": "/openapi.json"
    }))
}

async fn ready(State(pool): State<PgPool>) -> (StatusCode, Json<HealthStatus>) {
    let ready = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&pool)
        .await
        .is_ok();
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(HealthStatus {
            service: "governance-api".into(),
            ready,
        }),
    )
}

async fn openapi() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "openapi": "3.0.3", "info": {"title": "x402 Agent Spend Governance API", "version": env!("CARGO_PKG_VERSION")},
        "paths": {
            "/health/live":{"get":{}}, "/health/ready":{"get":{}}, "/v1/me":{"get":{"security":[{"bearerAuth":[]}]}},
            "/v1/agents":{"get":{"security":[{"bearerAuth":[]}]}}, "/v1/policies":{"get":{"security":[{"bearerAuth":[]}]}},
            "/v1/evaluate":{"post":{"security":[{"bearerAuth":[]}]}}, "/v1/settlements":{"post":{"security":[{"bearerAuth":[]}]}},
            "/v1/approvals":{"get":{"security":[{"bearerAuth":[]}]},"post":{"security":[{"bearerAuth":[]}]}},
            "/v1/audit/events":{"get":{"security":[{"bearerAuth":[]}]}}, "/v1/workspace/freeze":{"post":{"security":[{"bearerAuth":[]}]}},
            "/v1/exports":{"post":{"security":[{"bearerAuth":[]}]}},
            "/.well-known/governance-keys.json":{"get":{}},
            "/v1/decision-tokens/consume":{"post":{"security":[{"bearerAuth":[]}]}}
        },
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

async fn list_agents(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<AgentResponse>>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let agents = sqlx::query_as::<_, AgentResponse>(
        "SELECT agent_id, environment_id, name, status FROM control.agents WHERE workspace_id = $1 ORDER BY created_at ASC",
    )
    .bind(principal.workspace_id)
    .fetch_all(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(agents))
}

async fn list_policies(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<PolicySummary>>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let policies = sqlx::query_as::<_, PolicySummary>("SELECT policy_id, version, environment_id, status, created_at FROM policy.policy_versions WHERE workspace_id = $1 ORDER BY created_at DESC")
        .bind(principal.workspace_id).fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(policies))
}

async fn list_audit_events(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<AuditEventResponse>>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let events = sqlx::query_as::<_, AuditEventResponse>("SELECT event_id, aggregate_type, aggregate_id, sequence, event_type, payload, occurred_at, request_id FROM audit.events WHERE workspace_id = $1 ORDER BY occurred_at DESC LIMIT 100")
        .bind(principal.workspace_id).fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(events))
}

async fn create_settlement(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(request): Json<SettlementRequest>,
) -> Result<(StatusCode, Json<serde_json::Value>), StatusCode> {
    if request.idempotency_key.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let (intent_id,expected_amount,expected_pay_to,decision_outcome):(Uuid,String,Option<String>,String)=sqlx::query_as("SELECT i.intent_id,i.amount_atomic::text,i.merchant->>'pay_to',d.outcome FROM payments.decisions d JOIN payments.payment_intents i ON i.intent_id=d.intent_id WHERE d.decision_id=$1 AND i.workspace_id=$2 FOR UPDATE")
        .bind(request.decision_id).bind(principal.workspace_id).fetch_optional(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::NOT_FOUND)?;
    if decision_outcome != "allow" {
        return Err(StatusCode::CONFLICT);
    }
    if let Some(existing) = sqlx::query_scalar::<_, Uuid>("SELECT settlement_id FROM payments.settlements WHERE decision_id=$1 AND evidence->>'idempotency_key'=$2")
        .bind(request.decision_id).bind(&request.idempotency_key).fetch_optional(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        tx.rollback().await.ok();
        return Ok((StatusCode::OK, Json(serde_json::json!({"settlement_id":existing,"idempotent_replay":true}))));
    }
    let requested_outcome = match request.status {
        models::SettlementStatus::Settled => "settled",
        models::SettlementStatus::Failed => "failed",
        models::SettlementStatus::Unknown => "unknown",
    };
    let mismatch = requested_outcome == "settled"
        && (request.observed_amount_atomic.as_deref() != Some(expected_amount.as_str())
            || request.observed_pay_to.as_deref() != expected_pay_to.as_deref());
    let outcome = if mismatch {
        "mismatch"
    } else {
        requested_outcome
    };
    let id = Uuid::now_v7();
    let mut evidence = request.evidence;
    let object = evidence.as_object_mut().ok_or(StatusCode::BAD_REQUEST)?;
    object.insert(
        "idempotency_key".into(),
        serde_json::Value::String(request.idempotency_key),
    );
    object.insert(
        "expected_amount_atomic".into(),
        serde_json::Value::String(expected_amount.clone()),
    );
    let settlement_id: Uuid = sqlx::query_scalar("INSERT INTO payments.settlements (settlement_id,intent_id,decision_id,transaction_signature,facilitator_status,observed_amount_atomic,observed_pay_to,outcome,evidence) VALUES($1,$2,$3,$4,$5,$6::numeric,$7,$8,$9) ON CONFLICT (decision_id,transaction_signature) DO UPDATE SET evidence=payments.settlements.evidence RETURNING settlement_id")
        .bind(id).bind(intent_id).bind(request.decision_id).bind(request.transaction_signature).bind(requested_outcome).bind(request.observed_amount_atomic).bind(request.observed_pay_to).bind(outcome).bind(evidence).fetch_one(&mut *tx).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let reservation_status = match outcome {
        "settled" => "settled",
        "failed" => "failed",
        _ => "unknown",
    };
    let reservation=sqlx::query("UPDATE payments.budget_reservations SET status=$1, finalized_at=CASE WHEN $1 IN ('settled','failed') THEN now() ELSE finalized_at END WHERE decision_id=$2 AND status='reserved'").bind(reservation_status).bind(request.decision_id).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if reservation.rows_affected() != 1 && outcome != "mismatch" {
        return Err(StatusCode::CONFLICT);
    }
    if outcome == "failed" {
        sqlx::query("UPDATE payments.budget_counters c SET reserved_atomic=GREATEST(0,reserved_atomic-r.amount_atomic) FROM payments.budget_reservations r WHERE r.decision_id=$1 AND c.agent_id=(SELECT agent_id FROM payments.payment_intents WHERE intent_id=$2) AND c.window_type='daily' AND c.window_start=date_trunc('day',now())").bind(request.decision_id).bind(intent_id).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    if outcome == "unknown" || outcome == "mismatch" {
        sqlx::query("INSERT INTO jobs.work_items(job_id,job_type,payload) VALUES($1,'settlement_reconciliation',$2)").bind(Uuid::now_v7()).bind(serde_json::json!({"settlement_id":settlement_id,"decision_id":request.decision_id})).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok((
        StatusCode::OK,
        Json(serde_json::json!({"settlement_id": settlement_id, "outcome": outcome})),
    ))
}

async fn freeze_workspace(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(request): Json<FreezeWorkspaceRequest>,
) -> Result<StatusCode, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    auth::require_role(&principal, &[auth::Role::Owner, auth::Role::SecurityAdmin])
        .map_err(|_| StatusCode::FORBIDDEN)?;
    sqlx::query("UPDATE control.workspaces SET status = $1 WHERE workspace_id = $2")
        .bind(if request.frozen { "frozen" } else { "active" })
        .bind(principal.workspace_id)
        .execute(&pool)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_approvals(
    State(pool): State<PgPool>,
    headers: HeaderMap,
) -> Result<Json<Vec<ApprovalResponse>>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    auth::require_role(
        &principal,
        &[auth::Role::Owner, auth::Role::Approver, auth::Role::Auditor],
    )
    .map_err(|_| StatusCode::FORBIDDEN)?;
    let rows = sqlx::query_as::<_, ApprovalResponse>("SELECT a.approval_id, a.decision_id, a.status, a.requested_by, a.decided_by, a.expires_at, a.decided_at FROM payments.approvals a JOIN payments.decisions d ON d.decision_id = a.decision_id JOIN payments.payment_intents i ON i.intent_id = d.intent_id WHERE i.workspace_id = $1 ORDER BY a.expires_at ASC")
        .bind(principal.workspace_id).fetch_all(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(rows))
}

async fn decide_approval(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(request): Json<ApprovalDecisionRequest>,
) -> Result<Json<ApprovalResponse>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    auth::require_role(&principal, &[auth::Role::Owner, auth::Role::Approver])
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let row = sqlx::query_as::<_, ApprovalResponse>("UPDATE payments.approvals a SET status = $1, decided_by = $2, decided_at = now() FROM payments.decisions d, payments.payment_intents i WHERE a.decision_id = $3 AND d.decision_id = a.decision_id AND i.intent_id = d.intent_id AND i.workspace_id = $4 AND a.status = 'pending' AND a.expires_at > now() RETURNING a.approval_id, a.decision_id, a.status, a.requested_by, a.decided_by, a.expires_at, a.decided_at")
        .bind(if request.approved { "approved" } else { "denied" }).bind(principal.credential_id.to_string()).bind(request.decision_id).bind(principal.workspace_id).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::CONFLICT)?;
    Ok(Json(row))
}

async fn export_workspace(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(request): Json<ExportRequest>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    auth::require_role(&principal, &[auth::Role::Owner, auth::Role::Auditor])
        .map_err(|_| StatusCode::FORBIDDEN)?;
    let profile = match request.privacy_profile {
        PrivacyProfile::Internal => "internal",
        PrivacyProfile::Shareable => "shareable",
    };
    let export_id = Uuid::now_v7();
    sqlx::query("INSERT INTO jobs.work_items(job_id,job_type,payload) VALUES($1,'export_generation',$2)")
        .bind(Uuid::now_v7()).bind(serde_json::json!({"export_id":export_id,"workspace_id":principal.workspace_id,"environment_id":request.environment_id,"privacy_profile":profile})).execute(&pool).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({"export_id":export_id,"status":"queued","privacy_profile":profile}),
    ))
}

async fn not_found(
    request_id: Option<axum::extract::Extension<String>>,
) -> (StatusCode, Json<ErrorEnvelope>) {
    let request_id = request_id
        .map(|axum::extract::Extension(value)| value)
        .unwrap_or_else(|| Uuid::now_v7().to_string());
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
    let max_connections=env::var("DB_MAX_CONNECTIONS").ok().and_then(|v|v.parse().ok()).unwrap_or(10);
    let acquire_timeout_ms=env::var("DB_ACQUIRE_TIMEOUT_MS").ok().and_then(|v|v.parse().ok()).unwrap_or(5000);
    let pool = PgPoolOptions::new()
        .max_connections(max_connections)
        .acquire_timeout(std::time::Duration::from_millis(acquire_timeout_ms))
        .connect(&database_url)
        .await
        .expect("connect to PostgreSQL");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("run database migrations");
    let app = Router::new()
        .route("/", get(index))
        .route("/health/live", get(live))
        .route("/health/ready", get(ready))
        .route("/openapi.json", get(openapi))
        .route("/v1/me", get(me))
        .route("/v1/agents", get(list_agents))
        .route("/v1/policies", get(list_policies))
        .route("/v1/audit/events", get(list_audit_events))
        .route("/v1/settlements", post(create_settlement))
        .route("/v1/workspace/freeze", post(freeze_workspace))
        .route("/v1/approvals", get(list_approvals).post(decide_approval))
        .route("/v1/exports", post(export_workspace))
        .route("/v1/evaluate", post(evaluate::handler))
        .route(
            "/.well-known/governance-keys.json",
            get(tokens::public_keys),
        )
        .route("/v1/decision-tokens/consume", post(tokens::consume))
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

#[cfg(test)]
mod contract_tests {
    use super::*;
    use axum::extract::State;
    use axum::http::HeaderValue;
    use governance_core::AmountAtomic;

    #[tokio::test]
    async fn openapi_lists_every_phase_five_route() {
        let Json(document) = openapi().await;
        let paths = document["paths"].as_object().unwrap();
        for path in [
            "/v1/evaluate",
            "/v1/settlements",
            "/v1/agents",
            "/v1/policies",
            "/v1/approvals",
            "/v1/audit/events",
            "/v1/workspace/freeze",
            "/v1/exports",
            "/.well-known/governance-keys.json",
            "/v1/decision-tokens/consume",
        ] {
            assert!(paths.contains_key(path), "missing OpenAPI path {path}");
        }
    }

    #[tokio::test]
    async fn evaluate_is_database_backed_and_idempotent() {
        let Ok(url) = std::env::var("DATABASE_URL") else {
            return;
        };
        std::env::set_var("DECISION_KEY_ID", "phase6-test");
        std::env::set_var(
            "DECISION_SIGNING_KEY_B64",
            "CQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQkJCQk=",
        );
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect(&url)
            .await
            .unwrap();
        sqlx::migrate!("../../migrations").run(&pool).await.unwrap();
        let workspace = Uuid::now_v7();
        let environment = Uuid::now_v7();
        let agent = Uuid::now_v7();
        let credential = Uuid::now_v7();
        let policy = Uuid::now_v7();
        sqlx::query("INSERT INTO control.workspaces(workspace_id,name) VALUES($1,'phase5-test')")
            .bind(workspace)
            .execute(&pool)
            .await
            .unwrap();
        sqlx::query("INSERT INTO control.environments(environment_id,workspace_id,name) VALUES($1,$2,'test')").bind(environment).bind(workspace).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO control.agents(agent_id,workspace_id,environment_id,name) VALUES($1,$2,$3,'agent')").bind(agent).bind(workspace).bind(environment).execute(&pool).await.unwrap();
        let secret = "phase5-secret".to_owned();
        let hash = auth::credential_hash(&secret).unwrap();
        sqlx::query("INSERT INTO control.credentials(credential_id,workspace_id,agent_id,secret_hash,scopes) VALUES($1,$2,$3,$4,ARRAY['agent:operate'])").bind(credential).bind(workspace).bind(agent).bind(hash).execute(&pool).await.unwrap();
        let rules = serde_json::json!({"per_payment_cap":"100","daily_cap":"1000","approval_threshold":null,"allowed_networks":["solana-devnet"],"allowed_assets":["USDC"],"merchants":[{"origin":"https://merchant.test","pay_to":"merchant","path_prefix":null}]});
        sqlx::query("INSERT INTO policy.policy_versions(policy_id,version,workspace_id,environment_id,status,content_hash,rules,created_by,published_at) VALUES($1,1,$2,$3,'published',$4,$5,$6,now())").bind(policy).bind(workspace).bind(environment).bind(vec![1u8]).bind(rules).bind(credential).execute(&pool).await.unwrap();
        sqlx::query("INSERT INTO policy.active_policies(policy_id,workspace_id,environment_id,active_version) VALUES($1,$2,$3,1)").bind(policy).bind(workspace).bind(environment).execute(&pool).await.unwrap();
        let mut headers = HeaderMap::new();
        headers.insert(
            "authorization",
            HeaderValue::from_str(&format!("Bearer {credential}.{secret}")).unwrap(),
        );
        let quote_expires_at = chrono::Utc::now() + chrono::Duration::minutes(1);
        let make_request = || models::EvaluateRequest {
            idempotency_key: "same-key".into(),
            agent_id: agent,
            network: "solana-devnet".into(),
            asset: "USDC".into(),
            amount_atomic: AmountAtomic::new("10").unwrap(),
            merchant_origin: "https://merchant.test".into(),
            merchant_path_prefix: None,
            pay_to: "merchant".into(),
            request_hash: "sha256:test".into(),
            quote_expires_at,
        };
        let first = evaluate::handler(State(pool.clone()), headers.clone(), Json(make_request()))
            .await
            .unwrap()
            .0;
        let second = evaluate::handler(State(pool.clone()), headers.clone(), Json(make_request()))
            .await
            .unwrap()
            .0;
        assert_eq!(first.decision_id, second.decision_id);
        let token = first.decision_token.clone().unwrap();
        let intent_hash = String::from_utf8(
            sqlx::query_scalar::<_, Vec<u8>>(
                "SELECT intent_hash FROM payments.payment_intents WHERE intent_id=$1",
            )
            .bind(first.intent_id)
            .fetch_one(&pool)
            .await
            .unwrap(),
        )
        .unwrap();
        let verify = || models::VerifyDecisionTokenRequest {
            token: token.clone(),
            intent_hash: intent_hash.clone(),
            network: "solana-devnet".into(),
            asset: "USDC".into(),
            amount_atomic: "10".into(),
            pay_to: "merchant".into(),
            quote_expires_at,
        };
        assert!(
            tokens::consume(State(pool.clone()), headers.clone(), Json(verify()))
                .await
                .is_ok()
        );
        assert_eq!(
            tokens::consume(State(pool.clone()), headers, Json(verify()))
                .await
                .unwrap_err(),
            StatusCode::CONFLICT
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT count(*) FROM payments.payment_intents WHERE workspace_id=$1"
            )
            .bind(workspace)
            .fetch_one(&pool)
            .await
            .unwrap(),
            1
        );
    }
}
