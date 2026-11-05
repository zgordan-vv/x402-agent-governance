//! Domain boundary for x402 payment governance.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub type WorkspaceId = Uuid;
pub type AgentId = Uuid;
pub type IntentId = Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOutcome {
    Allow,
    Deny,
    ApprovalRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct HealthStatus {
    pub service: String,
    pub ready: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decision_outcome_serializes_stably() {
        assert_eq!(serde_json::to_string(&DecisionOutcome::ApprovalRequired).unwrap(), "\"approval_required\"");
    }
}
