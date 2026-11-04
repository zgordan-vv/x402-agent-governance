# ADR-004: Immutable Published Policy Versions

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Every payment decision must be explainable after the fact. An investigator must be able to answer which rules were active when an agent requested payment, even if the workspace owner later changes a limit or removes a merchant.

If the system edits a policy row in place, historical decisions become ambiguous: the same `policy_version` could mean different rules at different times. That breaks audit, incident response, replay, and customer trust.

## Decision

Published policies are immutable. A policy change creates a new version with a new monotonically increasing version number and a new content hash.

Policy lifecycle:

```text
draft → published → superseded
                 └→ frozen (if emergency-disabled)
```

Rules:

1. Draft policies can be edited by authorized roles.
2. Publishing validates schema and semantic invariants.
3. A published version cannot be updated or deleted.
4. Publishing a replacement marks the previous active version `superseded`.
5. A decision stores `policy_id`, `policy_version`, and `policy_hash`.
6. Audit records contain the same version and hash.
7. Historical decisions always resolve against the original immutable version.

The active version pointer may change atomically, but the version row itself never changes.

## Data model

```text
policy_id uuid
version integer
content_hash bytea
rules jsonb
status draft|published|superseded|frozen
created_by uuid
created_at timestamptz
published_at timestamptz
```

Recommended constraints:

```sql
PRIMARY KEY (policy_id, version)
UNIQUE (policy_id, content_hash)
CHECK (version > 0)
```

Application code must reject updates to `published`, `superseded`, and `frozen` rows. Database permissions should also prevent runtime roles from mutating published rule content.

## Publish transaction

Publishing uses one transaction:

1. lock the policy aggregate;
2. validate the draft;
3. allocate the next version;
4. calculate canonical content hash;
5. insert the immutable version;
6. atomically update the active-version pointer;
7. append an audit event;
8. commit.

No evaluation may observe an active pointer that refers to a partially published version.

## Why this matters

### Reproducible decisions

An operator can replay the exact rules that produced an allow or deny decision.

### Safe incident investigation

Changing a rule after an incident does not rewrite history or conceal the original authorization.

### Approval correctness

An approval references a concrete version. If that version is superseded before approval, the approval must be revalidated or denied rather than silently applying new rules.

### Deployment safety

Policy changes become reviewable artifacts with a diff, author, timestamp, and content hash.

## Rejected alternatives

### Mutable policy row

Rejected because historical decisions would depend on the current row rather than the rules used at decision time.

### Copy policy JSON into every decision only

Rejected as the sole model because it duplicates data and makes policy lifecycle, review, and active-version management difficult. Decisions still retain a hash and reference to the immutable version.

### Blockchain-only policy history

Rejected for MVP because private policy data, low-latency edits, and tenant access control do not belong on-chain. An optional external anchor may be added later.

## Consequences

Positive:

- deterministic audit and replay;
- clear policy diff and approval workflow;
- safe rollback by activating a previous known version;
- strong evidence for security reviews.

Negative:

- policy storage grows over time;
- publishing requires validation and version allocation;
- rollback is a new pointer change, not an in-place edit;
- UI must distinguish drafts from immutable versions.

## Required tests

- published rows reject updates;
- two concurrent publishes receive different versions;
- content hash changes when any rule changes;
- decisions keep resolving after policy replacement;
- an approval against a superseded version is revalidated;
- active pointer update and audit event are atomic;
- rollback activates a previous version without modifying it.

## Revisit when

Revisit only if a different policy storage model can preserve immutable historical replay, atomic activation, tenant isolation, and auditable version hashes with lower operational complexity.
