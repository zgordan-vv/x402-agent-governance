use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use ed25519_dalek::{Signature, Signer, SigningKey, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct DecisionTokenClaims {
    pub iss: String,
    pub decision_id: Uuid,
    pub intent_hash: String,
    pub workspace_id: Uuid,
    pub environment_id: Uuid,
    pub agent_id: Uuid,
    pub network: String,
    pub asset: String,
    pub amount_atomic: String,
    pub pay_to: String,
    pub policy_id: Uuid,
    pub policy_version: i32,
    pub iat: i64,
    pub exp: i64,
    pub nonce: Uuid,
    pub kid: String,
}

#[derive(Debug, Clone)]
pub struct ExpectedPayment<'a> {
    pub issuer: &'a str,
    pub workspace_id: Uuid,
    pub environment_id: Uuid,
    pub agent_id: Uuid,
    pub intent_hash: &'a str,
    pub network: &'a str,
    pub asset: &'a str,
    pub amount_atomic: &'a str,
    pub pay_to: &'a str,
    pub now: i64,
    pub quote_expires_at: i64,
    pub clock_skew_seconds: i64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum TokenError {
    #[error("token format is invalid")]
    InvalidFormat,
    #[error("token key is unknown")]
    UnknownKey,
    #[error("token signature is invalid")]
    InvalidSignature,
    #[error("token claims do not match the payment")]
    BindingMismatch,
    #[error("token is outside its validity window")]
    Expired,
    #[error("token nonce has already been consumed")]
    Replay,
}

pub struct DecisionTokenSigner {
    kid: String,
    key: SigningKey,
}

impl DecisionTokenSigner {
    pub fn new(kid: impl Into<String>, secret: [u8; 32]) -> Self {
        Self {
            kid: kid.into(),
            key: SigningKey::from_bytes(&secret),
        }
    }
    pub fn kid(&self) -> &str {
        &self.kid
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.verifying_key().to_bytes()
    }
    pub fn sign(&self, mut claims: DecisionTokenClaims) -> Result<String, TokenError> {
        claims.kid = self.kid.clone();
        let header = serde_json::json!({"alg":"EdDSA","typ":"GOVERNANCE+JWT","kid":self.kid});
        let h = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&header).map_err(|_| TokenError::InvalidFormat)?);
        let p = URL_SAFE_NO_PAD
            .encode(serde_json::to_vec(&claims).map_err(|_| TokenError::InvalidFormat)?);
        let input = format!("{h}.{p}");
        let signature = self.key.sign(input.as_bytes());
        Ok(format!(
            "{input}.{}",
            URL_SAFE_NO_PAD.encode(signature.to_bytes())
        ))
    }
}

#[derive(Default)]
pub struct DecisionTokenVerifier {
    keys: HashMap<String, VerifyingKey>,
}

impl DecisionTokenVerifier {
    pub fn add_key(&mut self, kid: impl Into<String>, public: [u8; 32]) -> Result<(), TokenError> {
        self.keys.insert(
            kid.into(),
            VerifyingKey::from_bytes(&public).map_err(|_| TokenError::InvalidFormat)?,
        );
        Ok(())
    }
    pub fn verify(
        &self,
        token: &str,
        expected: &ExpectedPayment<'_>,
        nonce_used: bool,
    ) -> Result<DecisionTokenClaims, TokenError> {
        let parts: Vec<_> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(TokenError::InvalidFormat);
        }
        let header: serde_json::Value = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(parts[0])
                .map_err(|_| TokenError::InvalidFormat)?,
        )
        .map_err(|_| TokenError::InvalidFormat)?;
        if header.get("alg").and_then(|v| v.as_str()) != Some("EdDSA")
            || header.get("typ").and_then(|v| v.as_str()) != Some("GOVERNANCE+JWT")
        {
            return Err(TokenError::InvalidFormat);
        }
        let kid = header
            .get("kid")
            .and_then(|v| v.as_str())
            .ok_or(TokenError::InvalidFormat)?;
        let key = self.keys.get(kid).ok_or(TokenError::UnknownKey)?;
        let sig_bytes = URL_SAFE_NO_PAD
            .decode(parts[2])
            .map_err(|_| TokenError::InvalidFormat)?;
        let signature = Signature::from_slice(&sig_bytes).map_err(|_| TokenError::InvalidFormat)?;
        key.verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature)
            .map_err(|_| TokenError::InvalidSignature)?;
        let claims: DecisionTokenClaims = serde_json::from_slice(
            &URL_SAFE_NO_PAD
                .decode(parts[1])
                .map_err(|_| TokenError::InvalidFormat)?,
        )
        .map_err(|_| TokenError::InvalidFormat)?;
        if nonce_used {
            return Err(TokenError::Replay);
        }
        if claims.iat > expected.now + expected.clock_skew_seconds
            || claims.exp < expected.now - expected.clock_skew_seconds
            || claims.exp > expected.quote_expires_at
        {
            return Err(TokenError::Expired);
        }
        if claims.kid != kid
            || claims.iss != expected.issuer
            || claims.workspace_id != expected.workspace_id
            || claims.environment_id != expected.environment_id
            || claims.agent_id != expected.agent_id
            || claims.intent_hash != expected.intent_hash
            || claims.network != expected.network
            || claims.asset != expected.asset
            || claims.amount_atomic != expected.amount_atomic
            || claims.pay_to != expected.pay_to
        {
            return Err(TokenError::BindingMismatch);
        }
        Ok(claims)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn claims() -> DecisionTokenClaims {
        DecisionTokenClaims {
            iss: "guardrail".into(),
            decision_id: Uuid::from_u128(1),
            intent_hash: "sha256:x".into(),
            workspace_id: Uuid::from_u128(2),
            environment_id: Uuid::from_u128(3),
            agent_id: Uuid::from_u128(4),
            network: "solana-devnet".into(),
            asset: "USDC".into(),
            amount_atomic: "10".into(),
            pay_to: "merchant".into(),
            policy_id: Uuid::from_u128(5),
            policy_version: 1,
            iat: 100,
            exp: 130,
            nonce: Uuid::from_u128(6),
            kid: String::new(),
        }
    }
    fn expected<'a>() -> ExpectedPayment<'a> {
        ExpectedPayment {
            issuer: "guardrail",
            workspace_id: Uuid::from_u128(2),
            environment_id: Uuid::from_u128(3),
            agent_id: Uuid::from_u128(4),
            intent_hash: "sha256:x",
            network: "solana-devnet",
            asset: "USDC",
            amount_atomic: "10",
            pay_to: "merchant",
            now: 110,
            quote_expires_at: 130,
            clock_skew_seconds: 2,
        }
    }
    #[test]
    fn valid_token_verifies() {
        let s = DecisionTokenSigner::new("k1", [7; 32]);
        let t = s.sign(claims()).unwrap();
        let mut v = DecisionTokenVerifier::default();
        v.add_key("k1", s.public_key()).unwrap();
        assert_eq!(
            v.verify(&t, &expected(), false).unwrap().decision_id,
            Uuid::from_u128(1)
        );
    }
    #[test]
    fn modified_fields_expiry_unknown_key_and_replay_fail() {
        let s = DecisionTokenSigner::new("k1", [7; 32]);
        let t = s.sign(claims()).unwrap();
        let mut v = DecisionTokenVerifier::default();
        v.add_key("k1", s.public_key()).unwrap();
        let mut e = expected();
        e.amount_atomic = "11";
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.now = 140;
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::Expired));
        assert_eq!(
            DecisionTokenVerifier::default().verify(&t, &expected(), false),
            Err(TokenError::UnknownKey)
        );
        assert_eq!(v.verify(&t, &expected(), true), Err(TokenError::Replay));
    }
    #[test]
    fn rotation_keeps_old_tokens_valid() {
        let old = DecisionTokenSigner::new("old", [1; 32]);
        let new = DecisionTokenSigner::new("new", [2; 32]);
        let token = old.sign(claims()).unwrap();
        let mut v = DecisionTokenVerifier::default();
        v.add_key("old", old.public_key()).unwrap();
        v.add_key("new", new.public_key()).unwrap();
        assert!(v.verify(&token, &expected(), false).is_ok());
    }
    #[test]
    fn every_payment_binding_is_exact() {
        let s = DecisionTokenSigner::new("k1", [7; 32]);
        let t = s.sign(claims()).unwrap();
        let mut v = DecisionTokenVerifier::default();
        v.add_key("k1", s.public_key()).unwrap();
        let mut e = expected();
        e.pay_to = "other";
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.asset = "SOL";
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.network = "mainnet";
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.intent_hash = "sha256:changed";
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.workspace_id = Uuid::from_u128(99);
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.environment_id = Uuid::from_u128(99);
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
        let mut e = expected();
        e.agent_id = Uuid::from_u128(99);
        assert_eq!(v.verify(&t, &e, false), Err(TokenError::BindingMismatch));
    }
    #[test]
    fn tampering_and_clock_skew_are_deterministic() {
        let s = DecisionTokenSigner::new("k1", [7; 32]);
        let t = s.sign(claims()).unwrap();
        let mut v = DecisionTokenVerifier::default();
        v.add_key("k1", s.public_key()).unwrap();
        let mut changed = t.clone();
        let last = changed.pop().unwrap();
        changed.push(if last == 'A' { 'B' } else { 'A' });
        assert!(v.verify(&changed, &expected(), false).is_err());
        let mut boundary = expected();
        boundary.now = 132;
        assert!(v.verify(&t, &boundary, false).is_ok());
        boundary.now = 133;
        assert_eq!(v.verify(&t, &boundary, false), Err(TokenError::Expired));
    }
}
