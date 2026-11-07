# Production acceptance report

Last verified: 2026-11-07

## Passed checks

| Check | Result | Evidence |
| --- | --- | --- |
| Rust workspace tests | Passed | 27 tests, 0 failures |
| TypeScript SDK tests | Passed | 5 tests, 0 failures |
| Typecheck | Passed | `pnpm typecheck` |
| Sandbox tests | Passed | 2 tests, 0 failures |
| Sandbox outcome demo | Passed | settled, rejected, mismatch, unknown |
| Environment comparison | Passed | sandbox and staging outcomes matched |
| Hosted smoke test | Passed | live, ready, metrics, OpenAPI all HTTP 200 |
| Hosted readiness load test | Passed | 1,000 requests, concurrency 50, 0 failures |
| Solana devnet observation | Passed | confirmed transaction, slot 519341618, no error |
| PostgreSQL backup check | Passed | latest host backup present and check script succeeded |
| Secret-pattern scan | Passed | no matching credentials in tracked files |
| npm dependency audit | Passed | no known production vulnerabilities |

## Not yet a production sign-off

- A clean Docker release build has not completed locally in this report; the
  first build requires a long uncached Rust compilation.
- `cargo-audit` and `cargo-deny` are enforced in GitHub Actions but are not
  installed in the local workstation environment.
- The devnet check verified RPC and on-chain observation; it did not create a
  new x402 API credential or execute a new governed payment in this run.
- No external pilot user has completed onboarding yet.

## Pilot gate

The system is suitable for a controlled sandbox/devnet pilot only. Before
accepting production funds, complete a cached Docker build, rerun the
PostgreSQL restart/worker recovery drills, and document an approved private RPC
and credential rotation procedure.
