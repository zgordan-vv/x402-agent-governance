use crate::{
    auth,
    models::{EvaluateRequest, EvaluateResponse},
    tokens,
};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use governance_core::token::DecisionTokenClaims;
use governance_core::{
    evaluate, AmountAtomic, Asset, BudgetSnapshot, DecisionOutcome, EvaluationContext,
    MerchantIdentity, Network, PaymentIntentInput, PolicyRules, ReasonCode,
};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

pub async fn handler(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(req): Json<EvaluateRequest>,
) -> Result<Json<EvaluateResponse>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    if principal.agent_id != Some(req.agent_id) || req.idempotency_key.trim().is_empty() {
        return Err(StatusCode::FORBIDDEN);
    }
    if let Some((decision_id,intent_id,outcome,reason,expires_at)) = sqlx::query_as::<_,(Uuid,Uuid,String,String,chrono::DateTime<chrono::Utc>)>("SELECT d.decision_id,i.intent_id,d.outcome,d.reason_code,d.expires_at FROM payments.payment_intents i JOIN payments.decisions d ON d.intent_id=i.intent_id WHERE i.workspace_id=$1 AND i.agent_id=$2 AND i.idempotency_key=$3")
        .bind(principal.workspace_id).bind(req.agent_id).bind(&req.idempotency_key).fetch_optional(&pool).await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)? {
        return Ok(Json(EvaluateResponse{decision_id,intent_id,outcome:parse_outcome(&outcome)?,reason_code:parse_reason(&reason)?,expires_at,decision_token:None}));
    }
    let environment_id = principal.environment_id.ok_or(StatusCode::FORBIDDEN)?;
    let (policy_id,version,rules_json):(Uuid,i32,serde_json::Value)=sqlx::query_as("SELECT p.policy_id,p.version,p.rules FROM policy.active_policies a JOIN policy.policy_versions p ON p.policy_id=a.policy_id AND p.version=a.active_version WHERE a.workspace_id=$1 AND a.environment_id=$2 AND p.status='published'")
        .bind(principal.workspace_id).bind(environment_id).fetch_optional(&pool).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.ok_or(StatusCode::CONFLICT)?;
    let rules: PolicyRules =
        serde_json::from_value(rules_json).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let remaining=sqlx::query_scalar::<_,String>("SELECT (limit_atomic-reserved_atomic)::text FROM payments.budget_counters WHERE workspace_id=$1 AND agent_id=$2 AND window_type='daily' AND window_start=date_trunc('day',now())")
        .bind(principal.workspace_id).bind(req.agent_id).fetch_optional(&pool).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.map(AmountAtomic::new).transpose().map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?.unwrap_or_else(||rules.daily_cap.clone());
    let intent = PaymentIntentInput {
        agent_id: req.agent_id,
        merchant: MerchantIdentity::normalize(
            &req.merchant_origin,
            req.merchant_path_prefix.as_deref(),
            &req.pay_to,
        )
        .map_err(|_| StatusCode::BAD_REQUEST)?,
        network: Network::new(&req.network).map_err(|_| StatusCode::BAD_REQUEST)?,
        asset: Asset::new(&req.asset).map_err(|_| StatusCode::BAD_REQUEST)?,
        amount_atomic: req.amount_atomic.clone(),
        request_hash: req.request_hash,
    };
    let frozen = sqlx::query_scalar::<_, bool>(
        "SELECT status='frozen' FROM control.workspaces WHERE workspace_id=$1",
    )
    .bind(principal.workspace_id)
    .fetch_one(&pool)
    .await
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let result = evaluate(&EvaluationContext {
        intent: intent.clone(),
        policy: rules.clone(),
        budget: BudgetSnapshot {
            daily_remaining: remaining,
        },
        frozen,
    })
    .map_err(|_| StatusCode::BAD_REQUEST)?;
    let intent_id = Uuid::now_v7();
    let decision_id = Uuid::now_v7();
    let now = chrono::Utc::now();
    if req.quote_expires_at <= now {
        return Err(StatusCode::UNPROCESSABLE_ENTITY);
    }
    let expires_at = std::cmp::min(now + chrono::Duration::seconds(30), req.quote_expires_at);
    let intent_hash = intent.intent_hash().map_err(|_| StatusCode::BAD_REQUEST)?;
    let decision_token = if result.outcome == DecisionOutcome::Allow {
        let signer = tokens::active_signer()?;
        Some(
            signer
                .sign(DecisionTokenClaims {
                    iss: "guardrail".into(),
                    decision_id,
                    intent_hash: intent_hash.clone(),
                    workspace_id: principal.workspace_id,
                    environment_id,
                    agent_id: req.agent_id,
                    network: intent.network.as_str().into(),
                    asset: intent.asset.as_str().into(),
                    amount_atomic: intent.amount_atomic.as_str().into(),
                    pay_to: intent.merchant.pay_to.clone(),
                    policy_id,
                    policy_version: version,
                    iat: now.timestamp(),
                    exp: expires_at.timestamp(),
                    nonce: Uuid::now_v7(),
                    kid: String::new(),
                })
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
        )
    } else {
        None
    };
    let token_hash = decision_token
        .as_ref()
        .map(|token| Sha256::digest(token.as_bytes()).to_vec());
    let token_kid = decision_token
        .as_ref()
        .map(|_| tokens::active_signer())
        .transpose()?
        .map(|s| s.kid().to_owned());
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    sqlx::query("INSERT INTO payments.payment_intents(intent_id,workspace_id,environment_id,agent_id,idempotency_key,intent_hash,merchant,amount_atomic,asset,network,status,expires_at) VALUES($1,$2,$3,$4,$5,$6,$7,$8::numeric,$9,$10,$11,$12)")
        .bind(intent_id).bind(principal.workspace_id).bind(environment_id).bind(req.agent_id).bind(req.idempotency_key).bind(intent_hash.as_bytes()).bind(serde_json::to_value(&intent.merchant).unwrap()).bind(intent.amount_atomic.as_str()).bind(intent.asset.as_str()).bind(intent.network.as_str()).bind(if result.outcome==DecisionOutcome::Deny{"denied"}else{"evaluated"}).bind(expires_at).execute(&mut *tx).await.map_err(|_|StatusCode::CONFLICT)?;
    sqlx::query("INSERT INTO payments.decisions(decision_id,intent_id,policy_id,policy_version,outcome,reason_code,explanation,token_key_id,token_hash,expires_at) VALUES($1,$2,$3,$4,$5,$6,$6,$7,$8,$9)")
        .bind(decision_id).bind(intent_id).bind(policy_id).bind(version).bind(outcome_str(&result.outcome)).bind(result.reason_code.as_str()).bind(token_kid).bind(token_hash).bind(expires_at).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if result.outcome == DecisionOutcome::Allow {
        sqlx::query("INSERT INTO payments.budget_counters(workspace_id,agent_id,window_type,window_start,limit_atomic,reserved_atomic) VALUES($1,$2,'daily',date_trunc('day',now()),$3::numeric,0) ON CONFLICT DO NOTHING").bind(principal.workspace_id).bind(req.agent_id).bind(rules.daily_cap.as_str()).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
        let updated=sqlx::query("UPDATE payments.budget_counters SET reserved_atomic=reserved_atomic+$3::numeric WHERE workspace_id=$1 AND agent_id=$2 AND window_type='daily' AND window_start=date_trunc('day',now()) AND reserved_atomic+$3::numeric<=limit_atomic").bind(principal.workspace_id).bind(req.agent_id).bind(intent.amount_atomic.as_str()).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
        if updated.rows_affected() != 1 {
            return Err(StatusCode::CONFLICT);
        }
        sqlx::query("INSERT INTO payments.budget_reservations(reservation_id,decision_id,amount_atomic,status,expires_at) VALUES($1,$2,$3::numeric,'reserved',$4)").bind(Uuid::now_v7()).bind(decision_id).bind(intent.amount_atomic.as_str()).bind(expires_at).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    } else if result.outcome == DecisionOutcome::ApprovalRequired {
        sqlx::query("INSERT INTO payments.approvals(approval_id,decision_id,status,requested_by,expires_at) VALUES($1,$2,'pending',$3,$4)").bind(Uuid::now_v7()).bind(decision_id).bind(principal.credential_id.to_string()).bind(expires_at).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(EvaluateResponse {
        decision_id,
        intent_id,
        outcome: result.outcome,
        reason_code: result.reason_code,
        expires_at,
        decision_token,
    }))
}
fn outcome_str(v: &DecisionOutcome) -> &'static str {
    match v {
        DecisionOutcome::Allow => "allow",
        DecisionOutcome::Deny => "deny",
        DecisionOutcome::ApprovalRequired => "approval_required",
    }
}
fn parse_outcome(v: &str) -> Result<DecisionOutcome, StatusCode> {
    match v {
        "allow" => Ok(DecisionOutcome::Allow),
        "deny" => Ok(DecisionOutcome::Deny),
        "approval_required" => Ok(DecisionOutcome::ApprovalRequired),
        _ => Err(StatusCode::INTERNAL_SERVER_ERROR),
    }
}
fn parse_reason(v: &str) -> Result<ReasonCode, StatusCode> {
    serde_json::from_value(serde_json::Value::String(v.into()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}
