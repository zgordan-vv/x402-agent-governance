# Technical Implementation Plan: x402 Agent Spend Governance

## Tech stack

- Rust, Tokio, Axum, Tower, Serde, SQLx, PostgreSQL 16, tracing, UUIDv7, SHA-256/BLAKE3, Ed25519, Argon2.
- TypeScript, Node.js 20+, Fetch API, React, Vite, React Router, TanStack Query, Zod.
- Docker Compose locally; multi-stage Docker images; managed PostgreSQL, object storage, HTTPS reverse proxy, GitHub Actions, OpenTelemetry-compatible metrics/traces.

## Repository layout

```text
crates/governance-core       domain types and deterministic evaluator
crates/governance-api        Axum routes and middleware
crates/governance-storage    SQLx repositories and transactions
crates/governance-worker     durable jobs and reconciliation
crates/governance-sdk-rust   Rust client
packages/x402-governance-sdk TypeScript SDK
apps/dashboard               React dashboard
migrations/                  PostgreSQL migrations
fixtures/                    safe fake quotes and test evidence
scripts/                     local/devnet/sandbox commands
docs/                        PRD, system design, ADRs, runbooks
```

## Phase 0 — Workspace bootstrap

Create the Cargo workspace, pnpm workspace, formatting/lint rules, `.env.example`, README, Docker Compose, healthchecks, and CI. Add `cargo fmt`, Clippy, Rust tests, TypeScript typecheck, lint, and frontend tests as required gates.

**Done when:** a clean checkout builds and starts PostgreSQL/API/dashboard locally.

## Phase 1 — Domain model and canonical protocol

Implement typed IDs, checked atomic amounts, network/asset enums, normalized merchant identity, canonical intent serialization, intent hashing, decision outcomes, and stable reason codes. Add tests for URL normalization, invalid values, canonical stability, and money overflow.

**Done when:** domain code has no HTTP or SQL dependency and all invariants have tests.

## Phase 2 — Policy schema and evaluator

Define policy versions and rules for caps, windows, allowlists, velocity, approval, expiry, and freeze. Implement a fixed evaluator order: identity, freeze, policy, network/asset, merchant, amount, budget, velocity, approval. Add unit/property tests for every allow and deny path.

**Done when:** identical input and policy always produce identical output and every reason code is covered.

## Phase 3 — PostgreSQL and storage

Create control, policy, payments, and audit schemas. Add tables for tenants, agents, credentials, policy versions, merchants, intents, decisions, counters, reservations, approvals, settlements, and audit events. Add foreign keys, unique constraints, checks, indexes, SQLx repositories, and the atomic evaluate-and-reserve transaction.

**Done when:** concurrent tests cannot overspend a budget and rollback leaves no partial state.

## Phase 4 — Authentication and authorization

Create one-time credentials, store only hashes, implement bearer middleware, revocation, expiry, workspace scoping, RBAC, request IDs, body limits, and rate limits. Test every route against every role and cross-tenant ID substitution.

**Done when:** no route bypasses backend authorization and revoked credentials fail immediately.

## Phase 5 — Axum API

Implement readiness/live endpoints and routes for evaluate, settlement, agents, policies, approvals, audit, freeze, and exports. Add typed request/response models, stable error envelopes, idempotency handling, OpenAPI generation, and contract tests.

**Done when:** API behavior is documented and tested without the dashboard.

## Phase 6 — Signed decision tokens

Implement Ed25519 key loading, public-key publication, key IDs, token claims, expiry, intent binding, nonce/replay protection, and rotation. Test modified amount/payTo, expired tokens, unknown keys, wrong hashes, and replay.

**Done when:** a token cannot authorize a different payment from the evaluated intent.

## Phase 7 — Settlement and reconciliation

Implement settlement evidence, the `reserved → settled/failed/unknown/released` state machine, mismatch detection, idempotent settlement writes, and reconciliation jobs for unknown outcomes.

**Done when:** an unknown or mismatched result can never be reported as successful.

## Phase 8 — TypeScript SDK

Implement typed client, 402 parser, `governedFetch`, injected signer interface, decision handling, settlement reporting, safe retry rules, and redaction. Add a fake merchant and tests proving deny occurs before signer invocation.

**Done when:** a basic x402 integration requires no more than 20 adapter lines.

## Phase 9 — Dashboard

Build React routes for overview, agents, policies, merchants, approvals, audit, and sandbox. Add typed API client, policy editor with diff/validation, approval confirmation, freeze controls, filters, pagination, and export status. Repeat all permissions in the API.

**Done when:** an operator can configure a policy and understand every decision without reading logs.

## Phase 10 — Worker and operational jobs

Add durable jobs for approval expiry, reservation expiry, unknown settlement reconciliation, exports, retention, and metric rollups. Implement leases, exponential backoff, max attempts, dead letters, and idempotent handlers.

**Done when:** a crashed worker can resume without duplicate state transitions.

## Phase 11 — Sandbox and test merchant

Create a no-money fake merchant that returns x402 quotes and simulates success, rejection, mismatch, and timeout. Package it in Compose with a one-command walkthrough.

**Done when:** the full allow/deny/approval/audit demo runs without real funds.

## Phase 12 — Security hardening

Write a threat model for stolen credentials, compromised agents, prompt injection, merchant spoofing, replay, race spending, database tampering, key rotation, and oversized requests. Run dependency, container, secret, authorization, replay, and cross-tenant scans.

**Done when:** no critical finding remains and negative paths fail closed.

## Phase 13 — Performance and reliability

Build a load generator. Measure p50/p95/p99 evaluation latency, database contention, audit write rate, and worker lag. Run 100 concurrent requests against one budget, database restart tests, signing-key outages, audit failures, worker crashes, facilitator timeouts, and backup/restore rehearsals.

**Done when:** SLO targets are measured and recovery behavior is documented.

## Phase 14 — Real devnet integration

Only after sandbox acceptance, configure a disposable devnet wallet, tiny test amounts, a facilitator adapter, and allow/deny/mismatch scenarios. Verify explorer signatures and revoke test credentials afterward.

**Done when:** real devnet payment evidence is linked to the correct decision without exposing keys.

## Phase 15 — Deployment

Build non-root multi-stage images. Configure secrets, TLS, managed PostgreSQL, restricted network access, backups, PITR, migrations, readiness checks, smoke tests, immutable releases, and rollback.

**Done when:** a clean environment can deploy and recover using runbooks.

## Phase 16 — Documentation and onboarding

Publish sandbox quickstart, SDK guide, policy cookbook, API reference, threat model, incident response, backup/restore, security questionnaire, ADR index, and customer onboarding. Every example must be runnable and secret-free.

**Done when:** a new developer reaches a safe sandbox result in under ten minutes.

## Phase 17 — Acceptance and release

Run functional, technical, security, performance, restore, and product acceptance. Verify allow/deny/approval/freeze, idempotency, mismatch detection, audit export, clean migrations, scans, secrets absence, adapter size, and three representative pilot workflows.

**Done when:** all acceptance criteria are signed off and the MVP release is reproducible from a clean checkout.

## Recommended execution order

Do not connect real money before Phase 11. The safe order is domain core, evaluator, database transaction, auth, API, tokens, settlement, SDK, sandbox, dashboard, worker, security, load/recovery, devnet, deployment, and release acceptance.
