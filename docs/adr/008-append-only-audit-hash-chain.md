# ADR-008: Append-Only Audit Log with Hash Chaining

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Guardrail is a security control plane. Customers must be able to determine who requested a payment, which policy version was used, why it was allowed or denied, whether an approval occurred, and what settlement result was later observed.

Ordinary mutable application logs are not sufficient evidence. An administrator or database mistake could update a decision explanation, delete a freeze event, or rewrite a policy-change timestamp without leaving a detectable trace.

## Decision

Guardrail stores security-relevant events in an append-only audit table. Events are never updated or deleted through the application API. Each event contains the hash of the previous event in its aggregate chain and its own canonical content hash.

```text
event_hash = SHA-256(
  previous_hash ||
  canonical_event_payload ||
  aggregate_id ||
  sequence ||
  occurred_at
)
```

An event includes:

- `event_id`;
- workspace and aggregate identity;
- monotonically increasing aggregate sequence;
- event type;
- actor type and actor ID;
- policy version and decision IDs where applicable;
- redacted payload;
- previous hash;
- event hash;
- occurrence timestamp;
- request/correlation ID.

## Events that must be recorded

- credential created, revoked, or expired;
- agent/workspace/policy freeze and unfreeze;
- policy draft published, superseded, or rolled back;
- intent evaluated and decision issued;
- approval requested, approved, expired, or denied;
- budget reserved, released, or finalized;
- settlement received, mismatched, failed, or marked unknown;
- audit export generated;
- administrative role or allowlist changes.

## Append transaction

An audit event is written in the same transaction as the state change it describes. For example, policy publishing atomically inserts the immutable version, moves the active pointer, and appends the publish event. An evaluation transaction atomically persists the intent, decision, reservation, and decision event.

The event sequence is allocated under an aggregate lock. Concurrent writers cannot produce duplicate sequence numbers or ambiguous ordering.

## Immutability enforcement

1. Runtime database roles have `INSERT` and `SELECT` but no `UPDATE`/`DELETE` on audit rows.
2. Application repositories expose append and query operations only.
3. Retention jobs archive partitions to controlled storage rather than editing history.
4. Exports include event hashes and chain metadata.
5. Verification tooling recomputes hashes from canonical payloads.

Hash chaining detects modification, but it is not a magic guarantee of honesty. Database access controls, backups, restore tests, and optional external anchoring remain necessary.

## Privacy rules

Audit payloads contain metadata needed to explain the decision, not secrets. They must not contain private keys, seed phrases, bearer tokens, raw Authorization headers, or complete paid response bodies. Request content is represented by a hash and safe labels.

## Rejected alternatives

### Ordinary mutable logs

Rejected because they are operational diagnostics, not durable security evidence.

### Blockchain-only audit

Rejected for MVP because policy, identity, and customer metadata should not be published on-chain. An optional anchor can publish a root hash later without exposing event contents.

### Full event sourcing

Rejected for MVP because rebuilding all projections and handling command idempotency would add complexity. The audit chain provides evidence while PostgreSQL remains the source of truth.

### One global hash chain

Rejected because it creates unnecessary cross-tenant coupling and a single write serialization point. Chains are scoped to aggregates or tenant streams and are linked through event IDs and timestamps.

## Consequences

Positive:

- policy and payment investigations are reproducible;
- tampering is detectable;
- exports can be independently verified;
- audit events align with transactional state changes;
- customer security reviews have a concrete evidence model.

Negative:

- audit storage grows continuously;
- canonical serialization must remain stable;
- retention and archive operations need care;
- hash integrity does not replace access control or backups;
- high event volume may require partitions and archive workers.

## Required tests

- every state-changing operation appends the expected event;
- event sequence is unique under concurrency;
- tampering with payload, sequence, or previous hash fails verification;
- audit and state rollback together on transaction failure;
- exports preserve hashes and omit secrets;
- archive/restore retains chain verification;
- tenant queries cannot read another tenant's events.

## Revisit when

Revisit if customers require an external notarization standard or a different immutable event store. Any replacement must preserve transactional correlation with decisions, tenant isolation, privacy, and independently verifiable integrity.
