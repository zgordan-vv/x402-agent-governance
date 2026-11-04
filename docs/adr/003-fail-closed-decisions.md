# ADR-003: Fail-Closed Payment Decisions

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Guardrail decides whether an autonomous agent may ask its customer-owned signer to sign a payment. If the policy database, evaluator, decision signer, or required identity data is unavailable, the service cannot prove that the payment is authorized.

Treating an unavailable control as an implicit approval would turn a temporary infrastructure failure into an unbounded spending path.

## Decision

Guardrail fails closed: when it cannot complete authentication, load an active policy, evaluate the intent, atomically reserve budget, or issue a valid decision token, it does not return `allow`.

The API returns one of:

- `deny` for a known policy violation;
- `approval_required` for a valid request needing human approval;
- `503 policy_unavailable` when the system cannot safely decide.

No response in the unavailable case contains a usable decision token. The SDK must not invoke the customer-owned signer for `deny` or `policy_unavailable`.

## Required behavior

1. Database timeout: return `503`, do not reserve or authorize.
2. Policy missing: return `deny/policy_not_configured`.
3. Policy expired: return `deny/policy_expired`.
4. Signing-key service unavailable: return `503`, even if evaluation passed.
5. Budget lock conflict: retry the database transaction safely; after the bounded retry budget, return `503`.
6. Freeze state unavailable: return `503`, never assume unfrozen.
7. Settlement timeout: record `unknown`; never mark success automatically.

## Rejected alternatives

### Fail open during outages

Rejected because it allows spending exactly when monitoring and controls are least reliable.

### Client-only fallback policy

Rejected as an authorization mechanism. A compromised agent can bypass a local fallback, and the server would lose independent evidence of the decision.

### Cached allow decision without authoritative reservation

Rejected because a stale cache can overspend a budget or ignore a freeze. Caches may accelerate reads, but the authoritative reservation remains transactional.

## Consequences

Positive:

- temporary outages cannot silently authorize payments;
- the security property is easy to explain and test;
- incident response starts from a bounded failure mode;
- customer-owned signers are called only after an explicit decision.

Negative:

- outages can pause otherwise legitimate agent work;
- availability targets require database and signing-key redundancy;
- SDKs need clear retry and operator messaging;
- unknown settlements require reconciliation.

## Operational requirements

- Expose readiness separately from liveness.
- Alert on `policy_unavailable`, database latency, signing-key errors, and deny spikes.
- Keep the outage response deterministic and documented.
- Never retry a signer call automatically after an ambiguous result.
- Provide operators with a controlled recovery path, not a bypass flag.

## Revisit when

Revisit only if a future architecture can preserve the same authorization guarantee with a formally verified local policy replica and safe atomic budget reservation. A generic “allow while offline” switch is not acceptable.
