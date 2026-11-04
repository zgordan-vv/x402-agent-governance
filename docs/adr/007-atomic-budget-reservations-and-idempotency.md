# ADR-007: Atomic Budget Reservations and Idempotency

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

An agent may retry a request because of a timeout, and multiple workers or concurrent tool calls may request payment at the same time. A check-then-write implementation can allow two requests to observe the same remaining budget and both spend it.

Payment APIs also need to distinguish a safe retry from a genuinely new payment. Without idempotency, a network retry can create a second intent, reservation, approval, or settlement.

## Decision

Every payment intent and settlement write is idempotent. Budget reservation, intent creation, decision creation, and the reservation audit event occur in one PostgreSQL transaction with locked counter rows.

The client supplies an `Idempotency-Key`. The server scopes it to workspace and agent:

```sql
UNIQUE (workspace_id, agent_id, idempotency_key)
```

The first request stores the canonical intent hash and outcome. A repeated request with the same key:

- returns the original result when the canonical hash matches;
- returns `idempotency_key_reused` when the hash differs;
- never creates a second reservation.

## Evaluate transaction

```text
BEGIN
  find existing intent by tenant + agent + idempotency key
  if found, compare canonical intent hash and return the stored result
  lock all applicable budget counter rows FOR UPDATE
  load active policy and freeze state
  run deterministic evaluator
  if outcome is allow or approval_required:
      increment reserved amount
      create reservation with expiry
  insert intent and immutable decision
  append audit event
COMMIT
```

If any operation fails, the transaction rolls back. The API must not reserve budget before the policy decision is durable, and must not persist an allow without its reservation.

## Reservation lifecycle

```text
reserved → settled
reserved → failed
reserved → unknown
reserved → released
```

Only one terminal transition is allowed. A scheduled job releases or reconciles expired reservations according to the configured policy. An unknown facilitator response is not a successful settlement.

## Isolation and locking

The initial implementation uses PostgreSQL row-level locks (`SELECT ... FOR UPDATE`) on one counter row per agent and budget window. The transaction isolation level and retry policy must handle serialization/deadlock errors without invoking the signer a second time.

If scale requires sharded counters, the same invariant remains: one authoritative atomic reservation operation must decide whether the requested amount fits.

## Settlement idempotency

Settlement uses a second idempotency key and unique decision/signature constraints. Repeating the same settlement request returns the stored outcome. A different transaction signature for the same decision creates a mismatch finding rather than another successful settlement.

## Rejected alternatives

### Check then update without a lock

Rejected because concurrent requests can overspend a budget.

### Redis-only counters

Rejected as authoritative state because failover, persistence, and reconciliation would be more difficult to prove. Redis may assist rate limiting, but PostgreSQL owns reservations.

### Client-generated “exactly once” behavior

Rejected because clients can crash, retry, or be compromised. The server must enforce idempotency independently.

### Automatically retrying a wallet signer

Rejected because an ambiguous signer response may mean the transaction was already signed or sent. The SDK may retry evaluation and settlement, but not an uncertain signing operation.

## Consequences

Positive:

- parallel calls cannot exceed a budget;
- network retries are safe;
- decisions and reservations remain consistent;
- duplicate settlements are observable;
- recovery jobs can reason about explicit states.

Negative:

- counter locks can create contention;
- callers must persist idempotency keys across retries;
- expired/unknown reservations require background reconciliation;
- transaction retry behavior must be carefully bounded.

## Required tests

- 100 concurrent requests never exceed a daily cap;
- repeated identical intent returns one decision and one reservation;
- reused key with changed amount is rejected;
- database rollback leaves no decision or reservation;
- deadlock/serialization retry remains idempotent;
- repeated settlement returns the original outcome;
- two signatures for one decision create a mismatch;
- expired reservation cannot be finalized twice.

## Revisit when

Revisit if the system adopts an external ledger that provides equivalent atomic compare-and-reserve, tenant isolation, durable idempotency, and auditable recovery semantics. A faster cache alone is not a replacement.
