# System Design: x402 Agent Spend Governance

## 1. Architecture summary

Guardrail is a non-custodial policy control plane. A TypeScript/Rust SDK sends a normalized payment intent to an Axum API. The API authenticates the agent, evaluates an immutable policy, reserves budget atomically in PostgreSQL, and returns an allow/deny/approval decision. Customer-owned code signs only after allow. Settlement evidence is recorded separately.

## 2. Functional requirements

The system must isolate workspaces and environments, manage scoped agent credentials, normalize merchants, evaluate network/asset/amount/budget/velocity/approval rules, issue bound decision tokens, process approvals, support freezes, reconcile settlements, and provide immutable audit queries and exports.

## 3. Non-functional requirements

- Default deny on missing policy or unavailable evaluator.
- p95 evaluation under 100 ms without external RPC; p99 under 300 ms.
- 99.5% MVP availability.
- Atomic counters and idempotent writes.
- TLS, RBAC, hashed credentials, secret redaction, signed tokens, and replay protection.
- No private keys or full paid responses in storage.
- Health endpoints, structured logs, metrics, tracing, backups, and restore rehearsal.

## 4. Capacity estimation

Pilot target: 100,000 intents/day, roughly 300 MB/day raw metadata at 3 KB per intent/decision/audit bundle, requiring approximately 100–150 GB for 90 days with indexes and WAL. A later scale target is 10 million intents/day, about 30 GB/day raw and multi-terabyte partitioned storage. The MVP starts below these targets and measures before scaling.

## 5. API design

All endpoints are versioned under `/v1`, use JSON, UUIDv7 IDs, bearer credentials, request IDs, and idempotency keys. Core endpoints are `/intents/evaluate`, `/intents/{id}/settlement`, `/policies`, `/agents`, `/approvals`, and `/audit/events`.

An intent contains agent/environment IDs, normalized merchant, network, asset, atomic amount, quote expiry, idempotency key, and request hash. An allow response contains decision ID, policy version, budget reservation, expiry, reason, and a signed decision token. Deny responses never contain a usable token.

## 6. High-level design

```text
AI agent + x402 SDK
        |
        v
API gateway -> authentication/validation/rate limits
        |
        v
Policy evaluator -> budget service -> approval service
        |
        v
PostgreSQL control/policy/payment/audit schemas
        |
allow token
        v
Customer-owned signer -> facilitator/merchant
        |
settlement evidence
        v
Settlement service -> audit and reconciliation worker
```

If policy, signer-token service, or database is unavailable, no allow token is issued. This is fail-closed.

## 7. Database design

Use schemas `control`, `policy`, `payments`, and `audit`. Core tables are workspaces, environments, agents, credentials, policy_versions, merchants, payment_intents, decisions, budget_counters, budget_reservations, approvals, settlements, and audit_events.

Important constraints:

```text
unique(workspace_id, agent_id, idempotency_key)
unique(aggregate_id, sequence)
amount >= 0
foreign keys for tenant ownership
immutable published policy versions
```

The evaluate transaction locks budget rows, runs the evaluator, inserts the intent/decision, reserves the amount, appends audit, and commits as one unit. Read replicas never authorize payments.

## 8. Detailed components

### Domain core

Rust types model IDs, atomic amounts, networks, assets, merchant identities, policies, decisions, and reason codes. Canonical serialization produces the intent hash.

### API gateway

Axum middleware handles authentication, request IDs, schema validation, body limits, CORS, timeouts, and error mapping. It does not implement policy logic.

### Policy evaluator

A deterministic Rust service checks identity, freeze, policy validity, network/asset, merchant, amount, budget, velocity, and approval in a fixed order. The first blocking reason is stable and machine-readable.

### Budget service

PostgreSQL row locking provides atomic session/daily/monthly reservations. Reservations move from `reserved` to `settled`, `failed`, `unknown`, or `released`; final states cannot move backward.

### Decision signer

An Ed25519 service key signs short-lived tokens containing intent hash, amount, payTo, network, policy version, expiry, nonce, and key ID. Rotation uses a public-key endpoint and does not affect customer wallets.

### Settlement service

Compares approved and observed amount, payTo, network, asset, and decision ID. Mismatch creates an audit finding and never becomes a successful settlement.

### Audit writer

Append-only events include actor, action, target, reason, policy version, correlation ID, sequence, previous hash, and event hash. Hash chaining detects modification but does not replace database ACL or backups.

### SDK

`governedFetch` parses a 402 response, evaluates the intent, stops before invoking the signer on deny, signs locally on allow, retries with payment proof, and submits settlement evidence. Automatic retries never repeat ambiguous signing.

### Dashboard

React screens cover overview, agents, policies, merchants, approvals, audit, and sandbox. The UI is not a security boundary; the API repeats all authorization checks.

### Worker

Durable jobs expire approvals, finalize expired reservations, reconcile unknown settlements, generate exports, and apply retention. Jobs use leases, retry/backoff, and dead-letter states.

## 9. Testing and operations

Unit and property tests cover evaluator precedence, canonical hashes, amount conversion, expiry, and merchant normalization. Integration tests cover PostgreSQL transactions, RBAC, approvals, freeze, settlement mismatch, and audit export. Security tests cover replay, cross-tenant IDs, race spending, prompt injection, and secret leakage. Load tests verify concurrency never exceeds a cap. Backups must be restored before release.
