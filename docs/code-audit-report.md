# Code Audit Report — Current Bootstrap

**Audit date:** 2026-11-05  
**Scope:** current repository after Phase 4  
**Method:** source inspection of Rust, SQL, TypeScript, Docker, and CI bootstrap code. This is a design/code audit, not a production security certification.

## Executive summary

The repository is still a bootstrap implementation. The domain primitives and policy evaluator are useful foundations, but the running API currently exposes only health endpoints, the dashboard/worker are placeholders, and the PostgreSQL migration is not wired into application startup. No memory leak was found in the small current codebase, but several security and correctness gaps must be closed before any real payment integration.

**Release recommendation:** do not connect real wallets, real funds, or production credentials yet.

## Findings

### AUD-001 — Critical — Payment/API authorization is not wired

**Evidence:** `crates/governance-api/src/main.rs` only exposes `/health/live` and `/health/ready`. `auth.rs` contains helper functions, but no route middleware verifies credentials against PostgreSQL or attaches a principal.

**Risk:** any future payment route could accidentally be deployed without authentication, tenant checks, or role enforcement.

**Remediation:** implement Axum authentication middleware, database credential lookup/revocation, scoped principal extraction, route-level RBAC, and integration tests that call every protected route unauthenticated and cross-tenant.

**Status:** Open; blocks Phase 5 and all payment functionality.

### AUD-002 — Critical — Database migrations are not executed

**Evidence:** `migrations/0001_initial.sql` exists, but no SQLx migrator or migration service is wired into API startup/Compose.

**Risk:** the application can start against an empty or incompatible database. A deployment may report healthy while payment tables and constraints do not exist.

**Remediation:** add a dedicated migration command/job, run migrations before readiness, make migration role separate from runtime role, and test clean and upgrade migrations.

**Status:** Open; blocks database-backed phases.

### AUD-003 — High — Credential hashing is vulnerable to offline brute force

**Evidence:** `auth::credential_hash` uses a single unsalted SHA-256 digest.

**Risk:** leaked credential hashes can be brute-forced cheaply, especially if issued tokens have predictable or insufficient entropy.

**Remediation:** use Argon2id with per-credential salt and constant-time verification. Keep a fast lookup prefix only as an index; do not use the prefix as authentication.

**Status:** Open; must be fixed before real credentials are issued.

### AUD-004 — High — No constant-time credential comparison or database verifier

**Evidence:** the current auth module only parses a bearer value and hashes it; it does not perform a constant-time comparison against a stored credential hash.

**Risk:** future implementers may compare secrets with ordinary string/byte equality or treat parsing as authentication.

**Remediation:** add a dedicated verifier using Argon2id, constant-time semantics, revocation/expiry checks, and negative tests for malformed, expired, and revoked credentials.

**Status:** Open.

### AUD-005 — High — Tenant relationships are not fully enforced by composite foreign keys

**Evidence:** `control.agents` references `workspace_id` and `environment_id` independently. `environment_id` is not constrained to belong to the same workspace. Similar independent references exist for policy and payment records.

**Risk:** inconsistent cross-tenant rows can be inserted if application checks are bypassed or a migration bug occurs.

**Remediation:** add composite unique keys and composite foreign keys such as `(workspace_id, environment_id)`, and enforce tenant ownership in repository queries and integration tests.

**Status:** Open; blocks production multi-tenancy.

### AUD-006 — High — Published policy immutability is not enforced at database level

**Evidence:** `policy.policy_versions.status` describes published rows, but the migration has no trigger or restricted role preventing updates/deletes to published content.

**Risk:** a privileged SQL path or future repository bug could rewrite policy history and invalidate audit replay.

**Remediation:** separate migration/runtime roles, revoke update/delete on policy versions for runtime, add a trigger that rejects mutation of published content, and test direct database attempts.

**Status:** Open.

### AUD-007 — High — Append-only audit is documented but not enforced

**Evidence:** `audit.events` has no database trigger or privilege policy preventing update/delete. The storage crate only exposes an insertion SQL fragment; it does not implement sequence locking or hash calculation.

**Risk:** audit evidence can be altered or written out of order by an incorrect repository or privileged runtime role.

**Remediation:** implement the append transaction, aggregate sequence lock, canonical hash calculation, runtime grants, verification tooling, and archive tests.

**Status:** Open.

### AUD-008 — Medium — Evaluator returns misleading reason codes

**Evidence:** frozen contexts return `policy_unavailable`, and network/asset mismatches return `merchant_not_allowlisted`.

**Risk:** operators and automated clients receive incorrect explanations, making remediation and metrics unreliable.

**Remediation:** add dedicated reason codes such as `workspace_frozen`, `agent_frozen`, `network_not_allowed`, and `asset_not_allowed`; update contract tests and dashboard copy.

**Status:** Open; fix before API contract publication.

### AUD-009 — Medium — Policy evaluator does not implement velocity or session/monthly budgets

**Evidence:** `PolicyRules` currently includes only per-payment cap, daily cap, approval threshold, network, asset, and merchant rules.

**Risk:** the implementation does not satisfy the PRD/system-design policy model and can under-enforce customer expectations.

**Remediation:** add session/monthly windows, velocity rules, and explicit budget reservation semantics before claiming MVP governance coverage.

**Status:** Open; planned in later Phase 2/3 work.

### AUD-010 — Medium — Amount handling has no checked arithmetic/reservation math

**Evidence:** `AmountAtomic` validates a decimal string and compares by normalized length/lexical order, but no addition/subtraction implementation exists.

**Risk:** future budget code may reimplement decimal arithmetic incorrectly or overflow native integers.

**Remediation:** introduce a checked arbitrary-precision amount type or use PostgreSQL `numeric(78,0)` with explicit conversion tests. Never use floating point.

**Status:** Open before atomic budget implementation.

### AUD-011 — Medium — Settlement uniqueness permits multiple NULL signatures

**Evidence:** `UNIQUE (decision_id, transaction_signature)` permits multiple rows with `NULL` in PostgreSQL.

**Risk:** repeated unknown settlement reports can create duplicate evidence rows for one decision.

**Remediation:** require a non-null idempotency key on settlements and add a unique `(decision_id, idempotency_key)` constraint. Use a nullable signature only as optional evidence.

**Status:** Open.

### AUD-012 — Medium — Compose contains insecure default database password

**Evidence:** `docker-compose.yml` defaults to `change-me-local-only`.

**Risk:** the default can accidentally be reused in a reachable deployment or copied into support scripts.

**Remediation:** keep local defaults bound to localhost only, fail startup when `APP_ENV=production` and the password is unchanged, and document secret-manager use. Never expose PostgreSQL publicly.

**Status:** Open; acceptable only for isolated local sandbox.

### AUD-013 — Medium — Request and policy size limits are missing

**Evidence:** no Axum body limit, policy rule count limit, merchant-list limit, or JSON depth/size limit is configured.

**Risk:** oversized requests can cause memory pressure, expensive parsing, or database bloat. This is a resource-exhaustion risk rather than a classic leak.

**Remediation:** add request body limits, bounded policy collections, database statement timeouts, and rate limits before public exposure.

**Status:** Open.

### AUD-014 — Medium — Dashboard and worker are placeholders

**Evidence:** dashboard only exports `bootstrap-ready`; Compose prints `dashboard bootstrap-ready`; worker only prints a bootstrap message.

**Risk:** the repository may appear to provide a working product while no UI, durable jobs, migration runner, or reconciliation exists.

**Remediation:** keep placeholder status explicit in README and implement real dashboard/worker before demo or production claims.

**Status:** Open by design; blocks product acceptance.

### AUD-015 — Low — API startup uses `expect` for runtime failures

**Evidence:** `main.rs` uses `expect` for address parsing, listener bind, and server failure.

**Risk:** the process exits without structured startup diagnostics or controlled shutdown behavior.

**Remediation:** initialize tracing, return structured startup errors, expose readiness only after dependencies are available, and handle graceful shutdown.

**Status:** Open; acceptable for Phase 0 but not production.

### AUD-016 — Low — No graceful shutdown or connection lifecycle management

**Evidence:** API calls `axum::serve(...).await` without shutdown signal handling; storage has only a pool wrapper and no migration/timeout configuration.

**Risk:** in-flight requests may be terminated abruptly and database connections may not drain during deployment.

**Remediation:** add Tokio signal handling, graceful Axum shutdown, pool limits/timeouts, and readiness drain behavior.

**Status:** Open.

## Memory-leak and resource review

No direct memory leak was found in the current small Rust codebase: there is no `unsafe`, `Box::leak`, raw pointer ownership, global unbounded cache, or background task loop retaining allocations. Rust ownership/RAII should reclaim normal request data.

However, resource-exhaustion risks remain:

- unbounded JSON/request bodies;
- unbounded policy merchant vectors;
- no database statement timeout;
- no connection-pool configuration;
- future audit exports could buffer large results in memory;
- future worker queues could grow without backpressure.

These must be addressed before load testing and public deployment.

## Performance review

Current code is too small for meaningful throughput benchmarking. The most important expected hotspots are:

1. budget row contention under concurrent agents;
2. audit writes in the evaluation transaction;
3. policy/merchant list scans rather than indexed normalized rules;
4. unbounded audit queries and exports;
5. future Argon2 credential verification cost.

Required measurements: evaluation p50/p95/p99, transaction lock wait, pool saturation, audit insert latency, authorization denial rate, and worker lag.

## Positive findings

- No unsafe Rust code found.
- Private keys are not accepted by the current API surface.
- Amounts are represented as decimal strings rather than floating point.
- Merchant userinfo is rejected.
- Intent hashing uses an explicit tuple rather than map iteration.
- Phase 0 CI includes formatting, Clippy, tests, and TypeScript typecheck.
- Docker runtime images use a non-root user.

## Recommended remediation order

1. Wire migrations and database readiness.
2. Implement real credential verification with Argon2id and constant-time checks.
3. Add backend auth/RBAC/tenant composite constraints.
4. Correct evaluator reason codes and complete budget rules.
5. Implement atomic reservation/idempotency and settlement constraints.
6. Enforce policy immutability and audit append-only permissions.
7. Add request limits, timeouts, graceful shutdown, and structured tracing.
8. Replace dashboard/worker placeholders.
9. Run concurrency, load, secret-scan, and restore tests.

## Audit limitations

This report covers the current repository snapshot only. It does not validate a deployed environment, cloud IAM, PostgreSQL runtime grants, container registry settings, DNS/TLS, or third-party facilitator behavior. Those require separate deployment and penetration tests.
