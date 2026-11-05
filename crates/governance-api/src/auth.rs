use axum::http::HeaderMap;
use governance_core::{AgentId, EnvironmentId, WorkspaceId};
use argon2::{password_hash::{PasswordHash, PasswordHasher, PasswordVerifier, SaltString}, Argon2};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role { Owner, SecurityAdmin, Operator, Approver, Auditor }

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Principal {
    pub credential_id: Uuid,
    pub workspace_id: WorkspaceId,
    pub environment_id: Option<EnvironmentId>,
    pub agent_id: Option<AgentId>,
    pub roles: Vec<Role>,
    pub revoked: bool,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthError {
    #[error("authorization header is missing")]
    MissingAuthorization,
    #[error("authorization scheme is invalid")]
    InvalidScheme,
    #[error("credential is empty")]
    EmptyCredential,
    #[error("credential is revoked")]
    RevokedCredential,
    #[error("role is not allowed")]
    Forbidden,
}

pub fn extract_bearer(headers: &HeaderMap) -> Result<&str, AuthError> {
    let value = headers.get("authorization").ok_or(AuthError::MissingAuthorization)?.to_str().map_err(|_| AuthError::InvalidScheme)?;
    let (scheme, credential) = value.split_once(' ').ok_or(AuthError::InvalidScheme)?;
    if !scheme.eq_ignore_ascii_case("bearer") { return Err(AuthError::InvalidScheme); }
    if credential.trim().is_empty() { return Err(AuthError::EmptyCredential); }
    Ok(credential.trim())
}

pub fn credential_hash(credential: &str) -> Result<String, AuthError> {
    if credential.trim().is_empty() { return Err(AuthError::EmptyCredential); }
    let salt = SaltString::generate(&mut argon2::password_hash::rand_core::OsRng);
    Ok(Argon2::default().hash_password(credential.as_bytes(), &salt).map_err(|_| AuthError::InvalidScheme)?.to_string())
}

pub fn verify_credential(credential: &str, encoded_hash: &str) -> Result<(), AuthError> {
    if credential.trim().is_empty() { return Err(AuthError::EmptyCredential); }
    let parsed = PasswordHash::new(encoded_hash).map_err(|_| AuthError::InvalidScheme)?;
    Argon2::default().verify_password(credential.as_bytes(), &parsed).map_err(|_| AuthError::Forbidden)
}

pub fn require_role(principal: &Principal, allowed: &[Role]) -> Result<(), AuthError> {
    if principal.revoked || !principal.roles.iter().any(|role| allowed.contains(role)) { return Err(if principal.revoked { AuthError::RevokedCredential } else { AuthError::Forbidden }); }
    Ok(())
}

pub fn same_workspace(principal: &Principal, workspace_id: WorkspaceId) -> Result<(), AuthError> {
    if principal.workspace_id == workspace_id { Ok(()) } else { Err(AuthError::Forbidden) }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    fn principal() -> Principal { Principal { credential_id: Uuid::nil(), workspace_id: Uuid::from_u128(1), environment_id: None, agent_id: None, roles: vec![Role::Operator], revoked: false } }

    #[test] fn extracts_bearer_case_insensitively() { let mut headers = HeaderMap::new(); headers.insert("authorization", HeaderValue::from_static("Bearer secret")); assert_eq!(extract_bearer(&headers).unwrap(), "secret"); }
    #[test] fn rejects_missing_or_wrong_scheme() { let headers = HeaderMap::new(); assert_eq!(extract_bearer(&headers), Err(AuthError::MissingAuthorization)); let mut headers = HeaderMap::new(); headers.insert("authorization", HeaderValue::from_static("Basic secret")); assert_eq!(extract_bearer(&headers), Err(AuthError::InvalidScheme)); }
    #[test] fn hashes_and_verifies_with_argon2id() { let hash = credential_hash("secret").unwrap(); assert!(hash.starts_with("$argon2")); assert!(verify_credential("secret", &hash).is_ok()); assert_eq!(verify_credential("wrong", &hash), Err(AuthError::Forbidden)); }
    #[test] fn role_and_tenant_checks_fail_closed() { let value = principal(); assert!(require_role(&value, &[Role::Operator]).is_ok()); assert_eq!(require_role(&value, &[Role::Owner]), Err(AuthError::Forbidden)); assert_eq!(same_workspace(&value, Uuid::from_u128(2)), Err(AuthError::Forbidden)); }
    #[test] fn revoked_principal_cannot_access_any_role() { let mut value = principal(); value.revoked = true; assert_eq!(require_role(&value, &[Role::Operator]), Err(AuthError::RevokedCredential)); }
}
