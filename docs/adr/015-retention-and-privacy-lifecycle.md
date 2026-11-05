# ADR-015: Retention and Privacy Lifecycle

- **Status:** Accepted
- **Date:** 2026-11-05

## Context

Guardrail needs enough evidence to investigate decisions and settlements, but payment metadata can reveal merchant relationships, agent behavior, request timing, and customer identifiers. Keeping every raw value forever increases privacy, storage, and breach impact.

Audit integrity also means that ordinary deletion must not silently rewrite the history used to explain a decision.

## Decision

Store the minimum data required for policy enforcement, reconciliation, audit, and configured reporting. Apply explicit retention classes:

| Data | Hot retention | Archive/delete policy |
|---|---:|---|
| Active policies and versions | lifetime of workspace | retain while referenced by decisions |
| Decisions and reason codes | customer-configured, default 90 days | archive redacted evidence, then delete metadata where permitted |
| Budget counters | current windows plus reconciliation window | compact historical counters |
| Settlement evidence | default 90 days | archive signature/status metadata |
| Audit events | default 1 year | partition/archive with hash metadata |
| Raw request/response content | never by default | only customer-approved hashes or redacted fields |
| Credentials | until revoked/expired plus bounded audit period | delete secret hash after retention requirement |

Published policy versions remain available as long as an active or retained decision references them. A customer may request workspace deletion subject to legal and contractual retention requirements.

## Data minimization

Do not store private keys, seed phrases, bearer tokens, raw authorization headers, paid response bodies, or unredacted prompt content. Store hashes and bounded metadata instead. URLs are normalized and may be redacted to origin/path classes according to the workspace privacy setting.

## Deletion and archival

Retention runs as a durable, auditable job:

1. select an eligible partition or record set;
2. verify it is outside legal/contractual holds;
3. export a redacted archive if configured;
4. preserve aggregate hash/checkpoint metadata;
5. delete or detach the eligible payload;
6. append a retention event without restoring deleted sensitive content;
7. verify the resulting archive/checkpoint.

Deletion cannot mutate an existing audit event. If an event payload is deleted for privacy, the system keeps a tombstone/checkpoint containing event ID, aggregate, sequence, deletion reason, and integrity hash where lawful.

## Tenant controls

Workspace owners can configure retention within platform bounds. A shorter customer retention period is allowed; a longer period requires an explicit plan/contract and storage estimate. Security administrators can place a legal or incident hold that pauses deletion.

## Rejected alternatives

### Keep all raw data forever

Rejected because it increases privacy, storage, and breach impact without being necessary for policy enforcement.

### Delete audit rows directly

Rejected because it would hide the fact that evidence was removed and could break integrity verification.

### Store full request and response bodies

Rejected because payment governance does not require content retention and responses may contain customer data or secrets.

### Rely on database TTL only

Rejected because retention needs holds, archive evidence, auditability, and different classes for different data.

## Consequences

Positive:

- reduced data exposure;
- predictable storage cost;
- customer-configurable lifecycle;
- explicit legal/incident holds;
- audit integrity remains verifiable around deletions.

Negative:

- retention workers and archive verification add complexity;
- a deleted payload may limit a later investigation;
- policy versions need reference tracking;
- privacy settings must be documented and tested.

## Required tests

- retention does not delete referenced active policy versions;
- legal/incident holds prevent deletion;
- raw secrets are rejected before persistence;
- archive/checkpoint verification succeeds;
- deletion creates the correct tombstone event;
- tenant owners cannot change another tenant's retention;
- restore of a retained archive preserves available hash evidence.

## Revisit when

Revisit if regulations, customer contracts, or external audit requirements impose different retention, deletion, or residency guarantees. Any change must preserve data minimization and explainable evidence lifecycle.
