# Code audit report — 2026-11-06

Scope: Rust API/worker/storage/core code, SQL migrations, TypeScript SDK, sandbox, scripts, and deployment configuration. This is a static review; it does not replace a penetration test or a production load test.

## Findings

### High — authentication performs an Argon2 verification against every credential

`crates/governance-api/src/auth.rs:99-108` loads every non-revoked credential and runs Argon2 verification for each request until one matches. The work is `O(number of credentials)` and each candidate is intentionally CPU-expensive. An unauthenticated caller can therefore consume database connections and CPU with repeated bearer requests; a workspace with many credentials also makes normal latency grow linearly.

Recommended fix: give credentials a public key ID/prefix, look up one hash by that identifier, then run exactly one Argon2 verification. Add a bounded per-process negative-result rate limiter and measure authentication latency.

### High — no application-level authentication rate limit

The API has a body limit and request timeout, but no per-IP, credential ID, or route rate limit. `/v1/evaluate` and authentication can be used to exhaust the pool or Argon2 workers before a reverse proxy notices.

Recommended fix: enforce limits at the edge and in the service for unauthenticated/authentication failures, with conservative `429` responses and metrics. Keep limits tenant-aware so one noisy tenant cannot starve others.

### Medium — worker pool settings are inconsistent with the API

`crates/governance-worker/src/main.rs:156-159` hard-codes five PostgreSQL connections and a five-second acquire timeout, while the API reads `DB_MAX_CONNECTIONS` and `DB_ACQUIRE_TIMEOUT_MS`. Production tuning can therefore change the API but silently leave the worker under- or over-provisioned.

Recommended fix: read the same validated environment variables in the worker, cap them to safe ranges, and document the combined connection budget.

### Medium — metric rollups repeatedly scan all historical payment rows

`crates/governance-worker/src/main.rs:31-34` runs three correlated `count(*)` queries per workspace over the full decisions and settlements tables every rollup bucket. Runtime and database I/O grow with total history, not with the current hour.

Recommended fix: aggregate by bounded time ranges using `created_at` indexes, or maintain an append-only hourly counter updated transactionally. Add an `EXPLAIN (ANALYZE, BUFFERS)` regression check at representative data volumes.

### Medium — retention cleanup is an unbounded delete

`crates/governance-worker/src/main.rs:26-29` deletes all old completed/dead-letter jobs in one statement. A large backlog can hold locks, generate a large WAL burst, and compete with payment traffic.

Recommended fix: delete in small primary-key batches (for example 500–2,000 rows), commit each batch, and expose progress/lag metrics. Apply the same bounded strategy to any future event retention job.

### Medium — settlement uniqueness is unsafe when `transaction_signature` is NULL

`migrations/0001_initial.sql:172` uses `UNIQUE (decision_id, transaction_signature)`, but PostgreSQL permits multiple NULL values. The API’s idempotency lookup normally prevents duplicates, yet concurrent requests with different idempotency keys and no signature can create multiple settlement rows for one decision.

Recommended fix: require a signature for settled/failed evidence, or add a partial unique index for `(decision_id) WHERE transaction_signature IS NULL` if only one unsigned/unknown record is allowed. Add a concurrent integration test.

### Low — startup failures use panics instead of structured diagnostics

Several `expect` calls in API/worker startup (`main.rs`) terminate the process, which is acceptable for unrecoverable configuration but produces less structured operational evidence than an error chain. Test helpers also use `unwrap` extensively.

Recommended fix: return `Result` from `main`, log the specific configuration/migration failure, and reserve `unwrap` for invariant-only test code.

## Positive controls observed

- SQL values are parameterized; no interpolated user input was found in query strings.
- Request body and request time limits are installed.
- Decision tokens are short-lived and bound to payment fields.
- Budget reservation and required decision persistence occur in a database transaction.
- Containers run as a non-root user and production overlay keeps PostgreSQL private.
- Secrets are not embedded in the repository; devnet tooling explicitly rejects private keys.

## Verification limits

This report did not execute a production penetration test, distributed rate-limit test, database failover, or a restore against customer-sized data. Those remain release-gate activities.
