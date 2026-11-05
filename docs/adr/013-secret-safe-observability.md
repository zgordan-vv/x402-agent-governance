# ADR-013: Secret-Safe Observability and Correlation

- **Status:** Accepted
- **Date:** 2026-11-05

## Context

Payment governance needs strong operational visibility. Operators must be able to connect an evaluation, budget reservation, decision, signer attempt, settlement, approval, and reconciliation job. At the same time, logs and telemetry are high-risk destinations for credentials, payment headers, request bodies, and wallet material.

Unstructured debug logs make incidents difficult to investigate. Logging raw requests can turn observability into a data-exfiltration path.

## Decision

Every request and asynchronous job receives a correlation context containing:

- `request_id`;
- `workspace_id`;
- `environment_id`;
- `agent_id` when available;
- `intent_id`;
- `decision_id`;
- `settlement_id` or `job_id` when available.

Services emit structured JSON logs through `tracing`. Metrics and traces use the same stable identifiers as attributes, with cardinality limits. Observability is secret-safe by construction: logs contain redacted metadata and hashes, never credentials or raw payment payloads.

## Required telemetry

### Logs

Record structured event name, outcome, reason code, latency, retry count, actor scope, and correlation IDs. Do not log bearer values, Authorization headers, private keys, seed phrases, request bodies, or complete merchant responses.

### Metrics

Expose:

```text
guardrail_decisions_total{outcome,reason_code}
guardrail_evaluation_duration_ms
guardrail_budget_conflicts_total
guardrail_settlement_mismatch_total
guardrail_approval_age_seconds
guardrail_audit_write_failures_total
guardrail_policy_unavailable_total
guardrail_worker_lag_seconds
```

Metric labels must be low-cardinality. Never use wallet address, intent ID, URL, or credential as a metric label.

### Traces

Trace spans cover authentication, policy loading, evaluation, budget transaction, token signing, settlement, and reconciliation. Trace attributes contain IDs and reason codes, not raw secrets or full quote payloads.

### Audit

Security evidence belongs in the append-only audit log, not only in ephemeral operational logs. Audit events follow ADR-008 and are queryable by authorized users.

## Redaction policy

Redact by field name and pattern:

- `authorization`, `cookie`, `token`, `secret`, `private_key`, `seed_phrase`, `mnemonic`;
- base58/base64 strings in known secret fields;
- full x402 payment headers when they contain proof material;
- request and response bodies unless explicitly allowlisted as safe test metadata.

Redaction occurs before the log event is serialized. A downstream log processor is not the only protection.

## Failure behavior

Observability failure must not turn a deny into an allow. If audit persistence is required by the decision transaction and cannot be written, the transaction rolls back. If an external metrics or trace exporter is down, the request may continue according to the payment safety rules, but the failure is counted locally and alerted.

## Rejected alternatives

### Raw request logging

Rejected because x402 headers and agent requests may contain payment proof, credentials, or sensitive customer data.

### IDs as metric labels

Rejected because high-cardinality labels can exhaust the metrics backend and expose sensitive identifiers.

### Logs as the only audit

Rejected because logs can be sampled, rotated, reordered, or altered and do not provide transactional evidence.

### Third-party tracing in the authorization decision

Rejected because an exporter outage must not determine whether a payment is authorized.

## Consequences

Positive:

- faster incident correlation;
- predictable dashboards and alerts;
- reduced secret-leak risk;
- clear distinction between operational logs and durable audit evidence;
- safe exporter outages.

Negative:

- redaction rules require maintenance;
- operators may need audit access for full context;
- low-cardinality metrics cannot answer every per-entity question;
- testing must verify telemetry privacy.

## Required tests

- secret-scanning finds no credentials in emitted logs;
- correlation IDs survive API, database, worker, and settlement paths;
- metrics remain bounded in cardinality;
- audit writes roll back the decision transaction on required-write failure;
- trace exporter outage does not create an authorization bypass;
- redaction covers known fields and representative secret patterns.

## Revisit when

Revisit if a compliance regime requires a different telemetry retention or external evidence format. Any replacement must preserve correlation, low-cardinality metrics, secret redaction, and transactional audit semantics.
