# ADR-010: Sandbox-First Rollout

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Guardrail controls payments made by autonomous software. A defect in policy evaluation, token binding, idempotency, or settlement reconciliation can cause a real loss. A portfolio/demo workflow must be useful without requiring a funded wallet, and customer teams need a safe place to test integration and failure handling.

Real devnet payments are useful for compatibility checks, but they are slower, externally dependent, and still involve keys and assets. They should not be the first feedback loop for every code change.

## Decision

Development and customer onboarding follow three progressively trusted environments:

1. **Local sandbox:** fake merchant, fake facilitator, no network funds.
2. **Devnet integration:** disposable wallet, tiny test amounts, real x402/facilitator/network behavior.
3. **Production:** only after security, load, backup/restore, and acceptance gates pass.

The local sandbox is the default for development, CI, documentation examples, and the public demo. It must simulate both successful and adverse outcomes without moving money.

## Sandbox requirements

The fake merchant must support:

- HTTP 402 quote generation;
- allow path;
- per-payment and merchant denials;
- approval-required path;
- facilitator rejection;
- settlement timeout/unknown result;
- amount/payTo mismatch;
- duplicate request and retry scenarios;
- deterministic fixture IDs for tests.

The sandbox must clearly label fake signatures and must never accept a real private key as configuration.

## Promotion gates

### Local sandbox to devnet

Require:

- all unit and integration tests passing;
- evaluator and budget concurrency tests passing;
- replay and token-binding tests passing;
- secret scan clean;
- dashboard demonstrates allow, deny, approval, freeze, and audit;
- backup/restore test for the sandbox database.

### Devnet to production

Require:

- real devnet allow/deny/mismatch scenarios;
- observed settlement evidence;
- security scan with no critical findings;
- load and failure-injection results against SLOs;
- production database restore rehearsal;
- reviewed runbooks and rollback plan;
- explicit customer or operator sign-off.

## Credential rules

- Sandbox credentials are scoped to sandbox and disposable.
- Devnet wallets are separate from production wallets.
- Production credentials are never copied into local examples, fixtures, or CI logs.
- Devnet test keys are revoked or destroyed after a test campaign.
- The dashboard must state which environment and whether money is involved.

## Why sandbox first

### Fast feedback

Most evaluator, API, UI, and reconciliation bugs do not require an actual blockchain transaction.

### Lower risk

No user funds or production credentials are exposed during normal development.

### Deterministic tests

Fake facilitator responses can reproduce timeouts, mismatches, and retries reliably.

### Better onboarding

A prospective customer can understand the product and integrate the SDK before funding a wallet.

## Rejected alternatives

### Devnet-only development

Rejected because external RPC/facilitator conditions make tests slow and flaky, while still exposing keys and assets.

### Production-first pilot

Rejected because policy and reconciliation defects could create irreversible financial loss.

### Static UI-only mockups

Rejected because they do not exercise the real evaluator, database transaction, SDK, or audit path. The sandbox must use the production code paths with a fake settlement boundary.

## Consequences

Positive:

- safe CI and documentation examples;
- deterministic failure testing;
- clear promotion gates;
- easier customer onboarding;
- real devnet compatibility remains available.

Negative:

- the fake merchant must faithfully model relevant x402 behavior;
- sandbox success does not prove blockchain settlement;
- two environments require explicit configuration and test coverage;
- operators must prevent accidental production credential use.

## Required tests

- a clean checkout can run the sandbox with one documented command;
- no sandbox test sends a real network transaction;
- all adverse outcomes are deterministic and visible in audit;
- environment separation rejects a sandbox credential against production;
- devnet tests use a disposable wallet and tiny amounts;
- promotion gates fail closed when a required suite is missing.

## Revisit when

Revisit only if a future protocol-standard test network provides equivalent deterministic, no-loss behavior. A real network may supplement the sandbox, but it must not replace the default safety boundary.
