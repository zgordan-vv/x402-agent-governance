//! PostgreSQL storage boundary. Domain logic stays in `governance-core`.

use chrono::{DateTime, Utc};
use governance_core::{AgentId, AmountAtomic, DecisionId, IntentId, PolicyId, WorkspaceId};
use serde_json::Value;
use sqlx::{PgPool, Postgres, Transaction};
use uuid::Uuid;

pub const STORAGE_STATUS: &str = "postgresql-ready";

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
    pub fn new(pool: PgPool) -> Self { Self { pool } }
    pub fn pool(&self) -> &PgPool { &self.pool }

    /// Starts the transaction used by the evaluate-and-reserve command.
    /// The actual evaluator is injected by the API layer; this boundary owns locks and writes.
    pub async fn begin_evaluation(&self) -> Result<Transaction<'_, Postgres>, sqlx::Error> {
        self.pool.begin().await
    }
}

/// SQL fragments kept next to the repository to make the transaction contract explicit.
pub mod queries {
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

#[allow(dead_code)]
fn _json_type_is_available(_: Value, _: Uuid) {}
