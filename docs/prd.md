# Product Requirements Document: x402 Agent Spend Governance

## 1. Product summary

Guardrail is a non-custodial policy gateway and SDK for autonomous x402 payments. It evaluates a payment intent before the customer-owned signer is called, returns an explainable allow/deny/approval decision, reserves budget atomically, and records settlement evidence afterward.

## 2. Problem

x402 describes how a server quotes a payment and how a client submits proof, but it does not define whether an agent is authorized to spend a particular amount with a particular merchant. Without an independent control layer, an agent can pay the wrong destination, exceed a budget, repeat a payment, or continue spending after compromise.

## 3. Goals

- Enforce payment policy before signing.
- Make every decision deterministic and explainable.
- Prevent budget races and duplicate intents.
- Preserve tamper-evident evidence.
- Integrate with existing x402 clients with minimal code.
- Keep customer keys and funds outside Guardrail.
- Provide sandbox, devnet, hosted, and self-hosted paths.

## 4. Out of scope for MVP

Custody, seed phrases, universal wallet functionality, refunds, legal dispute resolution, a facilitator, and LLM-only enforcement are out of scope. LLMs may later provide advisory anomaly detection but never the final authorization decision.

## 5. Users and roles

- **Workspace owner:** manages tenants, environments, policies, and members.
- **Security administrator:** manages allowlists, caps, rotation, and freezes.
- **Agent operator:** integrates the SDK and handles decisions.
- **Approver:** handles exceptional payments within a delegated scope.
- **Auditor:** reads and exports evidence without changing policy.

## 6. Core user flows

### Allowed payment

The SDK receives an x402 quote, normalizes merchant and payment data, calls `/v1/intents/evaluate`, receives `allow` and a short-lived decision token, invokes the customer-owned signer, sends proof to the facilitator, and submits settlement evidence.

### Denied payment

The evaluator returns a stable reason code such as `daily_budget_exceeded` or `merchant_not_allowlisted`. The SDK must not call the signer.

### Approval-required payment

A high-value request becomes pending. An authorized approver approves or denies it before expiry. Approval cannot change amount, merchant, network, or policy version.

### Emergency freeze

An owner or security administrator freezes an agent, environment, policy, or workspace. New intents fail closed. Already signed transactions cannot be cancelled and must be shown separately.

### Audit investigation

An auditor filters immutable decisions and settlements by agent, merchant, policy version, outcome, reason, and time, then exports redacted evidence.

## 7. Functional requirements

### Identity and scope

Every agent has a stable ID and scoped credential. Credentials expire and can be revoked. Agents cannot modify their own policy.

### Policy rules

Support per-payment, session, daily, and monthly caps; approval thresholds; network/asset restrictions; merchant origin/path/payTo allowlists; velocity limits; freeze/revoke; effective and expiry times; and default-deny behavior.

### Deterministic evaluator

The same normalized input and policy version always produce the same result. Evaluation order is identity, freeze, policy validity, network/asset, merchant, amount, budget, velocity, and approval.

### Intents and tokens

An intent contains an idempotency key, agent/environment IDs, merchant identity, network, asset, amount, quote expiry, and request hash. An allow token binds these values to a policy version, nonce, and expiry.

### Settlement evidence

Store decision ID, transaction signature/hash, facilitator result, observed amount/payTo, timestamps, and failure code. Unknown or mismatched settlement is never marked successful.

### Dashboard

Provide overview, agents, policies, merchants, approvals, usage, freezes, audit explorer, exports, and a no-money sandbox walkthrough.

## 8. API surface

```text
POST /v1/intents/evaluate
POST /v1/intents/{intent_id}/settlement
GET  /v1/decisions/{decision_id}
POST /v1/agents
POST /v1/agents/{agent_id}/credentials
POST /v1/policies
PUT  /v1/policies/{policy_id}
POST /v1/policies/{policy_id}/freeze
GET  /v1/approvals
POST /v1/approvals/{approval_id}/approve
POST /v1/approvals/{approval_id}/deny
GET  /v1/audit/events
POST /v1/audit/exports
```

All writes use idempotency where a retry is possible. Errors include `code`, `message`, `retryable`, and `request_id`.

## 9. Security and privacy

Use default-deny behavior, TLS, hashed credentials, RBAC, signed short-lived decisions, replay protection, secret redaction, immutable policy versions, and customer-owned signers. Do not store full paid responses, secret headers, private keys, or seed phrases.

## 10. MVP acceptance criteria

The MVP must allow a sandbox payment, block amount and merchant violations before signing, enforce a daily budget under parallel requests, make duplicates idempotent, support approval TTL, provide an effective kill switch, detect settlement mismatch, show policy version and reason in the dashboard, pass secret-scanning tests, and require no more than 20 adapter lines for a basic SDK integration.

## 11. Roadmap

1. Discovery and policy core.
2. Sandbox API and PostgreSQL persistence.
3. Real x402/devnet integration.
4. Approvals, RBAC, kill switch, audit export, and hardening.
5. Hosted/self-hosted packaging, SDKs, onboarding, support, and optional plans.
