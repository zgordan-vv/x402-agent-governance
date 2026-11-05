use governance_core::{AmountAtomic, DecisionOutcome, ReasonCode};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Serialize)]
pub struct ErrorEnvelope {
    pub error: ErrorBody,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody {
    pub code: &'static str,
    pub message: &'static str,
    pub request_id: String,
}

#[derive(Debug, Deserialize)]
pub struct EvaluateRequest {
    pub idempotency_key: String,
    pub agent_id: Uuid,
    pub network: String,
    pub asset: String,
    pub amount_atomic: AmountAtomic,
    pub merchant_origin: String,
    pub merchant_path_prefix: Option<String>,
    pub pay_to: String,
    pub request_hash: String,
}

#[derive(Debug, Serialize)]
pub struct EvaluateResponse {
    pub decision_id: Uuid,
    pub intent_id: Uuid,
    pub outcome: DecisionOutcome,
    pub reason_code: ReasonCode,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Deserialize)]
pub struct SettlementRequest {
    pub decision_id: Uuid,
    pub status: SettlementStatus,
    pub transaction_signature: Option<String>,
    pub evidence: serde_json::Value,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SettlementStatus {
    Settled,
    Failed,
    Unknown,
}

#[derive(Debug, Deserialize)]
pub struct ExportRequest {
    pub project_id: Uuid,
    pub privacy_profile: PrivacyProfile,
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct AgentResponse {
    pub agent_id: Uuid,
    pub environment_id: Uuid,
    pub name: String,
    pub status: String,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PrivacyProfile {
    Internal,
    Shareable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settlement_status_is_stable_snake_case() {
        assert_eq!(
            serde_json::to_string(&SettlementStatus::Unknown).unwrap(),
            "\"unknown\""
        );
    }
}
