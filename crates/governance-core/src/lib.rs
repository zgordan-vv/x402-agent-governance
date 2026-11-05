//! Domain types and canonical protocol primitives.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{fmt, str::FromStr};
use thiserror::Error;
use url::Url;
use uuid::Uuid;

pub type WorkspaceId = Uuid;
pub type EnvironmentId = Uuid;
pub type AgentId = Uuid;
pub type PolicyId = Uuid;
pub type IntentId = Uuid;
pub type DecisionId = Uuid;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum DomainError {
    #[error("amount must be a non-negative integer")]
    InvalidAmount,
    #[error("amount exceeds the supported atomic range")]
    AmountOverflow,
    #[error("merchant origin is invalid")]
    InvalidMerchant,
    #[error("merchant origin must not contain userinfo")]
    MerchantContainsUserinfo,
    #[error("merchant origin must use http or https")]
    UnsupportedMerchantScheme,
    #[error("network is empty")]
    EmptyNetwork,
    #[error("asset is empty")]
    EmptyAsset,
    #[error("request hash is empty")]
    EmptyRequestHash,
    #[error("policy rules are invalid")]
    InvalidPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AmountAtomic(String);

impl AmountAtomic {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
            return Err(DomainError::InvalidAmount);
        }
        if value.len() > 78 {
            return Err(DomainError::AmountOverflow);
        }
        let value = value.trim_start_matches('0');
        Ok(Self(if value.is_empty() { "0" } else { value }.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl fmt::Display for AmountAtomic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}
impl FromStr for AmountAtomic {
    type Err = DomainError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        Self::new(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Network(String);
impl Network {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyNetwork);
        }
        Ok(Self(value.trim().to_ascii_lowercase()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Asset(String);
impl Asset {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.trim().is_empty() {
            return Err(DomainError::EmptyAsset);
        }
        Ok(Self(value.trim().to_ascii_uppercase()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantIdentity {
    pub origin: String,
    pub path_prefix: Option<String>,
    pub pay_to: String,
}
impl MerchantIdentity {
    pub fn normalize(
        origin: &str,
        path_prefix: Option<&str>,
        pay_to: &str,
    ) -> Result<Self, DomainError> {
        let parsed = Url::parse(origin).map_err(|_| DomainError::InvalidMerchant)?;
        if !parsed.username().is_empty() || parsed.password().is_some() {
            return Err(DomainError::MerchantContainsUserinfo);
        }
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() {
            return Err(DomainError::UnsupportedMerchantScheme);
        }
        let mut normalized = format!(
            "{}://{}",
            parsed.scheme().to_ascii_lowercase(),
            parsed.host_str().unwrap().to_ascii_lowercase()
        );
        if let Some(port) = parsed.port() {
            let default = (parsed.scheme() == "http" && port == 80)
                || (parsed.scheme() == "https" && port == 443);
            if !default {
                normalized.push_str(&format!(":{port}"));
            }
        }
        let path = path_prefix.map(|value| {
            if value.starts_with('/') {
                value.to_owned()
            } else {
                format!("/{value}")
            }
        });
        if pay_to.trim().is_empty() {
            return Err(DomainError::InvalidMerchant);
        }
        Ok(Self {
            origin: normalized,
            path_prefix: path,
            pay_to: pay_to.trim().to_owned(),
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOutcome {
    Allow,
    Deny,
    ApprovalRequired,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode {
    WithinBudgetAndAllowlist,
    InvalidCredential,
    MerchantNotAllowlisted,
    NetworkNotAllowed,
    AssetNotAllowed,
    PerPaymentLimitExceeded,
    DailyBudgetExceeded,
    PolicyUnavailable,
    WorkspaceFrozen,
    ApprovalRequired,
}
impl ReasonCode {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::WithinBudgetAndAllowlist => "within_budget_and_allowlist",
            Self::InvalidCredential => "invalid_credential",
            Self::MerchantNotAllowlisted => "merchant_not_allowlisted",
            Self::NetworkNotAllowed => "network_not_allowed",
            Self::AssetNotAllowed => "asset_not_allowed",
            Self::PerPaymentLimitExceeded => "per_payment_limit_exceeded",
            Self::DailyBudgetExceeded => "daily_budget_exceeded",
            Self::PolicyUnavailable => "policy_unavailable",
            Self::WorkspaceFrozen => "workspace_frozen",
            Self::ApprovalRequired => "approval_required",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentIntentInput {
    pub agent_id: AgentId,
    pub merchant: MerchantIdentity,
    pub network: Network,
    pub asset: Asset,
    pub amount_atomic: AmountAtomic,
    pub request_hash: String,
}
impl PaymentIntentInput {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.request_hash.trim().is_empty() {
            return Err(DomainError::EmptyRequestHash);
        }
        Ok(())
    }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(&(
            self.agent_id,
            &self.merchant,
            &self.network,
            &self.asset,
            &self.amount_atomic,
            &self.request_hash,
        ))
    }
    pub fn intent_hash(&self) -> Result<String, serde_json::Error> {
        let digest = Sha256::digest(self.canonical_bytes()?);
        Ok(format!("sha256:{digest:x}"))
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus {
    pub service: String,
    pub ready: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MerchantRule {
    pub origin: String,
    pub pay_to: Option<String>,
    pub path_prefix: Option<String>,
}

impl MerchantRule {
    fn matches(&self, merchant: &MerchantIdentity) -> bool {
        self.origin == merchant.origin
            && self
                .pay_to
                .as_ref()
                .is_none_or(|pay_to| pay_to == &merchant.pay_to)
            && self.path_prefix.as_ref().is_none_or(|prefix| {
                merchant
                    .path_prefix
                    .as_ref()
                    .is_some_and(|path| path.starts_with(prefix))
            })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PolicyRules {
    pub per_payment_cap: AmountAtomic,
    pub daily_cap: AmountAtomic,
    pub approval_threshold: Option<AmountAtomic>,
    pub allowed_networks: Vec<Network>,
    pub allowed_assets: Vec<Asset>,
    pub merchants: Vec<MerchantRule>,
}

impl PolicyRules {
    pub fn validate(&self) -> Result<(), DomainError> {
        if self.allowed_networks.is_empty()
            || self.allowed_assets.is_empty()
            || self.merchants.is_empty()
        {
            return Err(DomainError::InvalidPolicy);
        }
        if compare_amount(&self.per_payment_cap, &self.daily_cap) == Ordering::Greater {
            return Err(DomainError::InvalidPolicy);
        }
        if self.approval_threshold.as_ref().is_some_and(|threshold| {
            compare_amount(threshold, &self.daily_cap) == Ordering::Greater
        }) {
            return Err(DomainError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BudgetSnapshot {
    pub daily_remaining: AmountAtomic,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationContext {
    pub intent: PaymentIntentInput,
    pub policy: PolicyRules,
    pub budget: BudgetSnapshot,
    pub frozen: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationResult {
    pub outcome: DecisionOutcome,
    pub reason_code: ReasonCode,
}

pub fn evaluate(context: &EvaluationContext) -> Result<EvaluationResult, DomainError> {
    context.intent.validate()?;
    context.policy.validate()?;
    if context.frozen {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::Deny,
            reason_code: ReasonCode::WorkspaceFrozen,
        });
    }
    if !context
        .policy
        .allowed_networks
        .contains(&context.intent.network)
    {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::Deny,
            reason_code: ReasonCode::NetworkNotAllowed,
        });
    }
    if !context
        .policy
        .allowed_assets
        .contains(&context.intent.asset)
    {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::Deny,
            reason_code: ReasonCode::AssetNotAllowed,
        });
    }
    if !context
        .policy
        .merchants
        .iter()
        .any(|merchant| merchant.matches(&context.intent.merchant))
    {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::Deny,
            reason_code: ReasonCode::MerchantNotAllowlisted,
        });
    }
    if compare_amount(
        &context.intent.amount_atomic,
        &context.policy.per_payment_cap,
    ) == Ordering::Greater
    {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::Deny,
            reason_code: ReasonCode::PerPaymentLimitExceeded,
        });
    }
    if compare_amount(
        &context.intent.amount_atomic,
        &context.budget.daily_remaining,
    ) == Ordering::Greater
    {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::Deny,
            reason_code: ReasonCode::DailyBudgetExceeded,
        });
    }
    if context
        .policy
        .approval_threshold
        .as_ref()
        .is_some_and(|threshold| {
            compare_amount(&context.intent.amount_atomic, threshold) != Ordering::Less
        })
    {
        return Ok(EvaluationResult {
            outcome: DecisionOutcome::ApprovalRequired,
            reason_code: ReasonCode::ApprovalRequired,
        });
    }
    Ok(EvaluationResult {
        outcome: DecisionOutcome::Allow,
        reason_code: ReasonCode::WithinBudgetAndAllowlist,
    })
}

use std::cmp::Ordering;

fn compare_amount(left: &AmountAtomic, right: &AmountAtomic) -> Ordering {
    left.as_str()
        .len()
        .cmp(&right.as_str().len())
        .then_with(|| left.as_str().cmp(right.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn intent() -> PaymentIntentInput {
        PaymentIntentInput {
            agent_id: Uuid::nil(),
            merchant: MerchantIdentity::normalize(
                "HTTPS://API.Example.com:443",
                Some("v1/data"),
                " merchant ",
            )
            .unwrap(),
            network: Network::new("Solana-Devnet").unwrap(),
            asset: Asset::new("usdc").unwrap(),
            amount_atomic: AmountAtomic::new("000250000").unwrap(),
            request_hash: "sha256:request".into(),
        }
    }
    #[test]
    fn amount_normalizes() {
        assert_eq!(AmountAtomic::new("00042").unwrap().as_str(), "42");
        assert_eq!(AmountAtomic::new("-1"), Err(DomainError::InvalidAmount));
    }
    #[test]
    fn merchant_normalizes() {
        let m = intent().merchant;
        assert_eq!(m.origin, "https://api.example.com");
        assert_eq!(m.path_prefix.as_deref(), Some("/v1/data"));
        assert_eq!(m.pay_to, "merchant");
    }
    #[test]
    fn merchant_rejects_unsafe_origin() {
        assert_eq!(
            MerchantIdentity::normalize("https://user:pass@example.com", None, "x"),
            Err(DomainError::MerchantContainsUserinfo)
        );
        assert_eq!(
            MerchantIdentity::normalize("ftp://example.com", None, "x"),
            Err(DomainError::UnsupportedMerchantScheme)
        );
    }
    #[test]
    fn canonical_hash_is_stable() {
        let a = intent().intent_hash().unwrap();
        assert_eq!(a, intent().intent_hash().unwrap());
        assert!(a.starts_with("sha256:"));
    }
    #[test]
    fn reason_codes_are_stable() {
        assert_eq!(
            ReasonCode::DailyBudgetExceeded.as_str(),
            "daily_budget_exceeded"
        );
        assert_eq!(
            serde_json::to_string(&DecisionOutcome::ApprovalRequired).unwrap(),
            "\"approval_required\""
        );
    }

    fn policy() -> PolicyRules {
        PolicyRules {
            per_payment_cap: AmountAtomic::new("100").unwrap(),
            daily_cap: AmountAtomic::new("1000").unwrap(),
            approval_threshold: Some(AmountAtomic::new("80").unwrap()),
            allowed_networks: vec![Network::new("solana-devnet").unwrap()],
            allowed_assets: vec![Asset::new("USDC").unwrap()],
            merchants: vec![MerchantRule {
                origin: "https://api.example.com".into(),
                pay_to: Some("merchant".into()),
                path_prefix: Some("/v1".into()),
            }],
        }
    }

    fn context(amount: &str) -> EvaluationContext {
        EvaluationContext {
            intent: PaymentIntentInput {
                amount_atomic: AmountAtomic::new(amount).unwrap(),
                ..intent()
            },
            policy: policy(),
            budget: BudgetSnapshot {
                daily_remaining: AmountAtomic::new("500").unwrap(),
            },
            frozen: false,
        }
    }

    #[test]
    fn evaluator_allows_normal_payment() {
        let result = evaluate(&context("20")).unwrap();
        assert_eq!(result.outcome, DecisionOutcome::Allow);
    }
    #[test]
    fn evaluator_denies_per_payment_limit_before_daily_limit() {
        let result = evaluate(&context("101")).unwrap();
        assert_eq!(result.reason_code, ReasonCode::PerPaymentLimitExceeded);
    }
    #[test]
    fn evaluator_denies_remaining_daily_budget() {
        let mut value = context("90");
        value.budget.daily_remaining = AmountAtomic::new("50").unwrap();
        assert_eq!(
            evaluate(&value).unwrap().reason_code,
            ReasonCode::DailyBudgetExceeded
        );
    }
    #[test]
    fn evaluator_requires_approval_at_threshold() {
        let result = evaluate(&context("80")).unwrap();
        assert_eq!(result.outcome, DecisionOutcome::ApprovalRequired);
    }
    #[test]
    fn evaluator_denies_frozen_context() {
        let mut value = context("20");
        value.frozen = true;
        assert_eq!(evaluate(&value).unwrap().outcome, DecisionOutcome::Deny);
    }
    #[test]
    fn invalid_policy_is_rejected() {
        let mut value = policy();
        value.per_payment_cap = AmountAtomic::new("1001").unwrap();
        assert_eq!(value.validate(), Err(DomainError::InvalidPolicy));
    }
}
