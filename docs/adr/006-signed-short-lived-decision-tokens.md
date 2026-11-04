# ADR-006: Signed, Short-Lived Decision Tokens

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

The policy API and the customer-owned signer are separate trust boundaries. A client needs a portable proof that Guardrail authorized one exact payment, while the signer or facilitator must be able to detect a changed amount, merchant, network, or expired authorization.

A plain database decision ID is not sufficient: it is mutable context without a cryptographically verifiable statement. A long-lived bearer authorization would also be replayable and too broad.

## Decision

For an `allow` result, Guardrail issues a short-lived Ed25519-signed decision token. The token authorizes only the evaluated intent and is not a blockchain payment signature.

Required claims:

```json
{
  "iss": "guardrail",
  "decision_id": "dec_01J...",
  "intent_hash": "sha256:...",
  "workspace_id": "ws_01J...",
  "environment_id": "env_01J...",
  "agent_id": "agent_researcher",
  "network": "solana-devnet",
  "asset": "USDC",
  "amount_atomic": "250000",
  "pay_to": "merchant-public-key",
  "policy_id": "pol_01J...",
  "policy_version": 12,
  "iat": 1793793600,
  "exp": 1793793630,
  "nonce": "unique-value",
  "kid": "decision-key-2026-01"
}
```

The default TTL is short (for example, 30 seconds) and is bounded by the original quote expiry. A token cannot extend a quote or a policy.

## Verification rules

The SDK/facilitator adapter verifies:

1. signature and trusted `kid`;
2. issuer and environment;
3. `iat`/`exp` and quote expiry;
4. intent hash;
5. exact amount and asset;
6. exact payTo and network;
7. decision status and nonce usage;
8. policy version still eligible for the operation.

Any mismatch is a hard failure. The verifier must not “repair” a token by reading a different amount from the current request.

## Key management

- The decision signing key is not a wallet key and cannot authorize funds.
- Production private material is loaded from KMS/secret manager.
- Public keys are exposed through a versioned key-discovery endpoint.
- `kid` identifies the active key.
- Rotation publishes a new key while retaining the old public key until all tokens signed with it expire.
- The runtime stores only a hash or metadata for tokens, not reusable plaintext tokens.

## Replay protection

The token includes a nonce and decision ID. The payment intent's idempotency key and decision status prevent multiple settlement records from creating multiple budget charges. A repeated submission may return the original outcome, but it must not create a second reservation.

Token validation is necessary but not sufficient: the authoritative API still checks decision state and budget reservation before finalizing settlement.

## Rejected alternatives

### Unsigned decision IDs

Rejected because a decision ID is not independently verifiable and can be substituted across contexts.

### Long-lived bearer tokens

Rejected because theft or replay would create a broad authorization window.

### Customer wallet signatures as policy proof

Rejected because a wallet signature proves possession of a key, not that Guardrail evaluated the payment under the intended policy.

### JWT with mutable policy lookup only

Rejected as the sole mechanism because it weakens binding and makes it harder to verify the exact rules and amount used at decision time. The token includes policy version and intent hash; the database remains authoritative for state.

## Consequences

Positive:

- portable cryptographic evidence of a specific allow;
- changed payment fields cannot reuse an approval;
- short TTL limits replay exposure;
- key rotation is independent of customer wallet keys;
- facilitator adapters can verify claims locally.

Negative:

- clock skew must be bounded and monitored;
- key discovery and rotation need operational support;
- a payment may require a new evaluation when a quote expires;
- token verification adds protocol complexity.

## Required tests

- valid token verifies;
- modified amount, payTo, asset, network, or intent hash fails;
- expired token fails;
- token from an unknown `kid` fails;
- token from another workspace/environment fails;
- replay does not create a second reservation;
- key rotation preserves verification until old-token expiry;
- clock-skew boundaries behave deterministically.

## Revisit when

Revisit if x402 standardizes an equivalent signed authorization object with the same binding, expiry, replay protection, and independent policy evidence. Until then, Guardrail-issued decision tokens remain the integration contract.
