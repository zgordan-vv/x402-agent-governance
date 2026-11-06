# Performance and reliability runbook

## Load generator

The repository includes a dependency-free load generator. It reports p50, p95,
p99 latency and failures without logging request headers or payment data.

```sh
LOAD_URL=http://127.0.0.1:8090/health \
LOAD_REQUESTS=1000 LOAD_CONCURRENCY=50 \
node scripts/load-test.mjs
```

Run it against `/health/ready` for API readiness or a safe sandbox route. Do
not point it at a funded merchant or real facilitator during load testing.

## Reliability drills

1. Start PostgreSQL, API, worker and sandbox with Compose.
2. Run the load generator and record p50/p95/p99 in the release report.
3. Stop/restart PostgreSQL. API readiness must become non-ready and recover
   after the database returns; no request may be converted into an allow.
4. Start API without `DECISION_SIGNING_KEY_B64`. An allow evaluation must fail
   closed with `503`; deny remains safe.
5. Stop the worker while an unknown settlement job exists, restart it, and
   verify the lease is recovered without a duplicate state transition.
6. Use the sandbox timeout scenario. The SDK must surface the timeout and never
   retry an uncertain signer operation.

## Configurable limits

`DB_MAX_CONNECTIONS` and `DB_ACQUIRE_TIMEOUT_MS` control API pool capacity.
The request body limit is 256 KiB and the API request timeout is 10 seconds.

## Backup and restore rehearsal

```sh
pg_dump "$DATABASE_URL" > /tmp/governance.sql
createdb governance_restore
psql governance_restore < /tmp/governance.sql
```

Run migrations and the workspace test suite against the restored database.
Never put credentials or signing keys in the dump command line or report.
