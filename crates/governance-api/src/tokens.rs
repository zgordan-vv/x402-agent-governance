use crate::{auth, models::VerifyDecisionTokenRequest};
use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    Json,
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use governance_core::token::{
    DecisionTokenSigner, DecisionTokenVerifier, ExpectedPayment, TokenError,
};
use serde_json::Value;
use sqlx::PgPool;
use std::{collections::BTreeMap, env};

pub fn active_signer() -> Result<DecisionTokenSigner, StatusCode> {
    let kid = env::var("DECISION_KEY_ID").map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let bytes = STANDARD
        .decode(env::var("DECISION_SIGNING_KEY_B64").map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?)
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let secret: [u8; 32] = bytes
        .try_into()
        .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    Ok(DecisionTokenSigner::new(kid, secret))
}

fn all_public_keys() -> Result<BTreeMap<String, [u8; 32]>, StatusCode> {
    let signer = active_signer()?;
    let mut keys = BTreeMap::new();
    keys.insert(signer.kid().to_owned(), signer.public_key());
    if let Ok(raw) = env::var("DECISION_OLD_PUBLIC_KEYS_JSON") {
        let old: BTreeMap<String, String> =
            serde_json::from_str(&raw).map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
        for (kid, value) in old {
            let bytes = STANDARD
                .decode(value)
                .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
            keys.insert(
                kid,
                bytes
                    .try_into()
                    .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?,
            );
        }
    }
    Ok(keys)
}

pub async fn public_keys() -> Result<Json<Value>, StatusCode> {
    let active = env::var("DECISION_KEY_ID").map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    let keys=all_public_keys()?.into_iter().map(|(kid,key)|serde_json::json!({"kid":kid,"alg":"EdDSA","kty":"OKP","crv":"Ed25519","x":URL_SAFE_NO_PAD.encode(key)})).collect::<Vec<_>>();
    Ok(Json(serde_json::json!({"active_kid":active,"keys":keys})))
}

pub async fn consume(
    State(pool): State<PgPool>,
    headers: HeaderMap,
    Json(request): Json<VerifyDecisionTokenRequest>,
) -> Result<Json<Value>, StatusCode> {
    let principal = auth::authenticate(&pool, &headers)
        .await
        .map_err(|_| StatusCode::UNAUTHORIZED)?;
    let environment = principal.environment_id.ok_or(StatusCode::FORBIDDEN)?;
    let agent = principal.agent_id.ok_or(StatusCode::FORBIDDEN)?;
    let mut verifier = DecisionTokenVerifier::default();
    for (kid, key) in all_public_keys()? {
        verifier
            .add_key(kid, key)
            .map_err(|_| StatusCode::SERVICE_UNAVAILABLE)?;
    }
    let expected = ExpectedPayment {
        issuer: "guardrail",
        workspace_id: principal.workspace_id,
        environment_id: environment,
        agent_id: agent,
        intent_hash: &request.intent_hash,
        network: &request.network,
        asset: &request.asset,
        amount_atomic: &request.amount_atomic,
        pay_to: &request.pay_to,
        now: chrono::Utc::now().timestamp(),
        quote_expires_at: request.quote_expires_at.timestamp(),
        clock_skew_seconds: 2,
    };
    let claims = verifier
        .verify(&request.token, &expected, false)
        .map_err(token_status)?;
    let mut tx = pool
        .begin()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let valid=sqlx::query_scalar::<_,String>("SELECT d.outcome FROM payments.decisions d JOIN payments.payment_intents i ON i.intent_id=d.intent_id JOIN policy.policy_versions p ON p.policy_id=d.policy_id AND p.version=d.policy_version JOIN policy.active_policies a ON a.policy_id=d.policy_id AND a.active_version=d.policy_version WHERE d.decision_id=$1 AND i.workspace_id=$2 AND i.agent_id=$3 AND p.status='published' AND d.expires_at>=now() FOR UPDATE").bind(claims.decision_id).bind(principal.workspace_id).bind(agent).fetch_optional(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?==Some("allow".into());
    if !valid {
        return Err(StatusCode::CONFLICT);
    }
    let inserted=sqlx::query("INSERT INTO payments.decision_token_nonces(nonce,decision_id,workspace_id,expires_at) VALUES($1,$2,$3,to_timestamp($4)) ON CONFLICT DO NOTHING").bind(claims.nonce).bind(claims.decision_id).bind(principal.workspace_id).bind(claims.exp as f64).execute(&mut *tx).await.map_err(|_|StatusCode::INTERNAL_SERVER_ERROR)?;
    if inserted.rows_affected() != 1 {
        return Err(StatusCode::CONFLICT);
    }
    tx.commit()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(
        serde_json::json!({"valid":true,"decision_id":claims.decision_id,"nonce":claims.nonce}),
    ))
}

fn token_status(error: TokenError) -> StatusCode {
    match error {
        TokenError::UnknownKey => StatusCode::UNAUTHORIZED,
        TokenError::Replay => StatusCode::CONFLICT,
        TokenError::Expired
        | TokenError::BindingMismatch
        | TokenError::InvalidFormat
        | TokenError::InvalidSignature => StatusCode::UNPROCESSABLE_ENTITY,
    }
}
