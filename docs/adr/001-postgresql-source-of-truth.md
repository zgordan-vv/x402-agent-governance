# ADR-001: PostgreSQL as the Source of Truth

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Guardrail must evaluate payments before signing while maintaining budgets, idempotency, approvals, settlements, and audit evidence. Concurrent requests must not spend the same budget twice, and a decision must never exist without its reservation and audit record.

## Decision

PostgreSQL 16+ is the authoritative store for workspaces, agents, credential metadata, immutable policy versions, merchants, intents, decisions, budget counters, reservations, approvals, settlements, and audit events.

Evaluation and reservation use one transaction:

```text
BEGIN
  return an existing intent for the idempotency key, if present
  lock the relevant budget counter rows
  load active policy and freeze state
  evaluate the normalized intent
  insert the intent and immutable decision
  reserve budget when applicable
  append the audit event
COMMIT
```

Any failed check rolls back the complete operation. Redis may be used for cache or rate limiting, replicas for read-heavy audit queries, and object storage for exports, but none may authorize a payment or replace authoritative budget state.

## Rationale

- ACID transactions provide the required decision/reservation boundary.
- Unique constraints enforce idempotency and aggregate ordering.
- Foreign keys and check constraints provide a second safety layer.
- Relational queries fit tenant-scoped audit, usage, approval, and reconciliation views.
- Managed PostgreSQL provides mature TLS, backups, PITR, replicas, and role separation.
- One database keeps the MVP operationally simple.

## Rejected alternatives

Redis-only, MongoDB, Kafka/event log, blockchain, and event-sourcing-only designs were rejected for the core ledger. They can serve specialized purposes later, but do not provide the simplest authoritative transaction boundary for budgets, decisions, and audit.

## Consequences

Positive: atomic reservations, strong idempotency, simple local Docker setup, mature backup/restore tooling, and useful audit queries.

Negative: PostgreSQL is on the critical evaluation path; high write volume eventually requires partitioning and replicas; audit retention increases storage; multi-region active-active writes are not an MVP feature; database outage must fail closed for new payments.

## Safeguards

1. Separate runtime and migration roles.
2. Use TLS and secret-manager-managed credentials.
3. Keep authoritative checks on the primary, never on a stale read replica.
4. Test concurrent reservations and rollback behavior.
5. Partition and archive old audit data.
6. Rehearse backup restore before production release.

## Revisit when

Revisit this decision if active-active multi-region writes, more than 10,000 evaluations/sec per tenant, or storage volumes beyond practical PostgreSQL partitioning become real requirements.
