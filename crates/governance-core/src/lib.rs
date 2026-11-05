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
    #[error("amount must be a non-negative integer")] InvalidAmount,
    #[error("amount exceeds the supported atomic range")] AmountOverflow,
    #[error("merchant origin is invalid")] InvalidMerchant,
    #[error("merchant origin must not contain userinfo")] MerchantContainsUserinfo,
    #[error("merchant origin must use http or https")] UnsupportedMerchantScheme,
    #[error("network is empty")] EmptyNetwork,
    #[error("asset is empty")] EmptyAsset,
    #[error("request hash is empty")] EmptyRequestHash,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AmountAtomic(String);

impl AmountAtomic {
    pub fn new(value: impl Into<String>) -> Result<Self, DomainError> {
        let value = value.into();
        if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) { return Err(DomainError::InvalidAmount); }
        if value.len() > 78 { return Err(DomainError::AmountOverflow); }
        let value = value.trim_start_matches('0');
        Ok(Self(if value.is_empty() { "0" } else { value }.to_owned()))
    }
    pub fn as_str(&self) -> &str { &self.0 }
}
impl fmt::Display for AmountAtomic { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(&self.0) } }
impl FromStr for AmountAtomic { type Err = DomainError; fn from_str(value: &str) -> Result<Self, Self::Err> { Self::new(value) } }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Network(String);
impl Network { pub fn new(value: impl Into<String>) -> Result<Self, DomainError> { let value = value.into(); if value.trim().is_empty() { return Err(DomainError::EmptyNetwork); } Ok(Self(value.trim().to_ascii_lowercase())) } }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Asset(String);
impl Asset { pub fn new(value: impl Into<String>) -> Result<Self, DomainError> { let value = value.into(); if value.trim().is_empty() { return Err(DomainError::EmptyAsset); } Ok(Self(value.trim().to_ascii_uppercase())) } }

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MerchantIdentity { pub origin: String, pub path_prefix: Option<String>, pub pay_to: String }
impl MerchantIdentity {
    pub fn normalize(origin: &str, path_prefix: Option<&str>, pay_to: &str) -> Result<Self, DomainError> {
        let parsed = Url::parse(origin).map_err(|_| DomainError::InvalidMerchant)?;
        if !parsed.username().is_empty() || parsed.password().is_some() { return Err(DomainError::MerchantContainsUserinfo); }
        if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none() { return Err(DomainError::UnsupportedMerchantScheme); }
        let mut normalized = format!("{}://{}", parsed.scheme().to_ascii_lowercase(), parsed.host_str().unwrap().to_ascii_lowercase());
        if let Some(port) = parsed.port() { let default = (parsed.scheme() == "http" && port == 80) || (parsed.scheme() == "https" && port == 443); if !default { normalized.push_str(&format!(":{port}")); } }
        let path = path_prefix.map(|value| if value.starts_with('/') { value.to_owned() } else { format!("/{value}") });
        if pay_to.trim().is_empty() { return Err(DomainError::InvalidMerchant); }
        Ok(Self { origin: normalized, path_prefix: path, pay_to: pay_to.trim().to_owned() })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionOutcome { Allow, Deny, ApprovalRequired }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ReasonCode { WithinBudgetAndAllowlist, InvalidCredential, MerchantNotAllowlisted, PerPaymentLimitExceeded, DailyBudgetExceeded, PolicyUnavailable, ApprovalRequired }
impl ReasonCode { pub fn as_str(&self) -> &'static str { match self { Self::WithinBudgetAndAllowlist => "within_budget_and_allowlist", Self::InvalidCredential => "invalid_credential", Self::MerchantNotAllowlisted => "merchant_not_allowlisted", Self::PerPaymentLimitExceeded => "per_payment_limit_exceeded", Self::DailyBudgetExceeded => "daily_budget_exceeded", Self::PolicyUnavailable => "policy_unavailable", Self::ApprovalRequired => "approval_required" } } }

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PaymentIntentInput { pub agent_id: AgentId, pub merchant: MerchantIdentity, pub network: Network, pub asset: Asset, pub amount_atomic: AmountAtomic, pub request_hash: String }
impl PaymentIntentInput {
    pub fn validate(&self) -> Result<(), DomainError> { if self.request_hash.trim().is_empty() { return Err(DomainError::EmptyRequestHash); } Ok(()) }
    pub fn canonical_bytes(&self) -> Result<Vec<u8>, serde_json::Error> { serde_json::to_vec(&(self.agent_id, &self.merchant, &self.network, &self.asset, &self.amount_atomic, &self.request_hash)) }
    pub fn intent_hash(&self) -> Result<String, serde_json::Error> { let digest = Sha256::digest(self.canonical_bytes()?); Ok(format!("sha256:{digest:x}")) }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthStatus { pub service: String, pub ready: bool }

#[cfg(test)]
mod tests {
    use super::*;
    fn intent() -> PaymentIntentInput { PaymentIntentInput { agent_id: Uuid::nil(), merchant: MerchantIdentity::normalize("HTTPS://API.Example.com:443", Some("v1/data"), " merchant ").unwrap(), network: Network::new("Solana-Devnet").unwrap(), asset: Asset::new("usdc").unwrap(), amount_atomic: AmountAtomic::new("000250000").unwrap(), request_hash: "sha256:request".into() } }
    #[test] fn amount_normalizes() { assert_eq!(AmountAtomic::new("00042").unwrap().as_str(), "42"); assert_eq!(AmountAtomic::new("-1"), Err(DomainError::InvalidAmount)); }
    #[test] fn merchant_normalizes() { let m = intent().merchant; assert_eq!(m.origin, "https://api.example.com"); assert_eq!(m.path_prefix.as_deref(), Some("/v1/data")); assert_eq!(m.pay_to, "merchant"); }
    #[test] fn merchant_rejects_unsafe_origin() { assert_eq!(MerchantIdentity::normalize("https://user:pass@example.com", None, "x"), Err(DomainError::MerchantContainsUserinfo)); assert_eq!(MerchantIdentity::normalize("ftp://example.com", None, "x"), Err(DomainError::UnsupportedMerchantScheme)); }
    #[test] fn canonical_hash_is_stable() { let a = intent().intent_hash().unwrap(); assert_eq!(a, intent().intent_hash().unwrap()); assert!(a.starts_with("sha256:")); }
    #[test] fn reason_codes_are_stable() { assert_eq!(ReasonCode::DailyBudgetExceeded.as_str(), "daily_budget_exceeded"); assert_eq!(serde_json::to_string(&DecisionOutcome::ApprovalRequired).unwrap(), "\"approval_required\""); }
}
