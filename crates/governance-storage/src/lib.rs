//! PostgreSQL storage boundary. Domain logic stays in `governance-core`.

use chrono::{DateTime, Utc};
use governance_core::{AgentId, AmountAtomic, DecisionId, IntentId, PolicyId, WorkspaceId};
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub const STORAGE_STATUS: &str = "postgresql-ready";

/// Computes the stable hash used by the append-only audit chain.
pub fn audit_event_hash(
    previous_hash: Option<&[u8]>,
    aggregate_id: Uuid,
    sequence: i64,
    event_type: &str,
    payload: &Value,
    occurred_at: DateTime<Utc>,
) -> Vec<u8> {
    let canonical = serde_json::to_vec(&(
        previous_hash,
        aggregate_id,
        sequence,
        event_type,
        payload,
        occurred_at.to_rfc3339(),
    ))
    .expect("audit payload must serialize");
    Sha256::digest(canonical).to_vec()
}

pub fn verify_audit_event_hash(
    previous_hash: Option<&[u8]>,
    aggregate_id: Uuid,
    sequence: i64,
    event_type: &str,
    payload: &Value,
    occurred_at: DateTime<Utc>,
    expected: &[u8],
) -> bool {
    audit_event_hash(
        previous_hash,
        aggregate_id,
        sequence,
        event_type,
        payload,
        occurred_at,
    )
    .as_slice()
        == expected
}

#[derive(Debug, Clone)]
pub struct IntentRecord {
    pub intent_id: IntentId,
    pub workspace_id: WorkspaceId,
    pub agent_id: AgentId,
    pub idempotency_key: String,
    pub intent_hash: Vec<u8>,
    pub amount_atomic: AmountAtomic,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone)]
pub struct DecisionRecord {
    pub decision_id: DecisionId,
    pub intent_id: IntentId,
    pub policy_id: PolicyId,
    pub policy_version: i32,
    pub outcome: String,
    pub reason_code: String,
}

/// Repository boundary for the authoritative PostgreSQL transaction.
pub struct Storage {
    pool: PgPool,
}

impl Storage {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Starts the transaction used by the evaluate-and-reserve command.
    /// The actual evaluator is injected by the API layer; this boundary owns locks and writes.
    pub async fn begin_evaluation(&self) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        self.pool.begin().await
    }
}

/// SQL fragments kept next to the repository to make the transaction contract explicit.
pub mod queries {
    pub const LOCK_AUDIT_AGGREGATE: &str =
        "SELECT pg_advisory_xact_lock(hashtextextended($1::text, 0))";

    pub const READ_AUDIT_TAIL: &str = "SELECT COALESCE(MAX(sequence), 0), (array_agg(event_hash ORDER BY sequence DESC))[1] FROM audit.events WHERE aggregate_id = $1";

    pub const RESERVE_BUDGET: &str = r#"
        UPDATE payments.budget_counters
        SET reserved_atomic = reserved_atomic + $4
        WHERE workspace_id = $1 AND agent_id = $2 AND window_start = $3
          AND reserved_atomic + $4 <= limit_atomic
        RETURNING reserved_atomic, limit_atomic
    "#;

    pub const INSERT_RESERVATION: &str = r#"
        INSERT INTO payments.budget_reservations
          (reservation_id, decision_id, amount_atomic, status, expires_at)
        VALUES ($1, $2, $3, 'reserved', $4)
    "#;

    pub const LOCK_BUDGET_COUNTER: &str = r#"
        SELECT agent_id, window_start, reserved_atomic, limit_atomic
        FROM payments.budget_counters
        WHERE workspace_id = $1 AND agent_id = $2 AND window_start = $3
        FOR UPDATE
    "#;

    pub const FIND_IDEMPOTENT_INTENT: &str = r#"
        SELECT intent_id, intent_hash, status
        FROM payments.payment_intents
        WHERE workspace_id = $1 AND agent_id = $2 AND idempotency_key = $3
    "#;

    pub const INSERT_AUDIT_EVENT: &str = r#"
        INSERT INTO audit.events
          (event_id, workspace_id, aggregate_type, aggregate_id, sequence,
           event_type, actor_type, actor_id, payload, prev_hash, event_hash,
           occurred_at, request_id)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
    "#;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn audit_hash_is_deterministic_and_detects_payload_changes() {
        let id = Uuid::from_u128(7);
        let when = DateTime::parse_from_rfc3339("2026-11-05T00:00:00Z")
            .unwrap()
            .with_timezone(&Utc);
        let payload = serde_json::json!({"decision": "allow"});
        let hash = audit_event_hash(None, id, 1, "decision.created", &payload, when);
        assert!(verify_audit_event_hash(
            None,
            id,
            1,
            "decision.created",
            &payload,
            when,
            &hash
        ));
        assert!(!verify_audit_event_hash(
            None,
            id,
            1,
            "decision.created",
            &serde_json::json!({"decision": "deny"}),
            when,
            &hash
        ));
    }
}

#[allow(dead_code)]
fn _json_type_is_available(_: Value, _: Uuid) {}
