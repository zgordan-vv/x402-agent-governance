# ADR-005: Deterministic Policy Evaluation, Optional LLM Advice

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Guardrail may eventually use machine learning to identify unusual spending patterns, but the primary authorization decision controls real funds. Authorization must be reproducible, bounded, low-latency, and explainable to an operator or auditor.

An LLM is probabilistic. Its output can vary with model versions, prompts, context ordering, provider availability, and adversarial input. A model must not be able to turn a prohibited payment into an allowed one.

## Decision

The final payment decision is made by a deterministic, versioned policy evaluator. The evaluator checks identity, freeze state, policy validity, network, asset, merchant, amount, budget, velocity, and approval requirements in a fixed order.

An optional anomaly detector may run after or alongside deterministic evaluation. It can:

- add an advisory risk signal;
- suggest a policy review;
- recommend human approval;
- enrich an audit report.

It cannot:

- override a deny;
- create an allow without deterministic checks;
- change a budget counter;
- change a merchant allowlist;
- mutate policy;
- receive private keys or secrets.

## Evaluation contract

```rust
pub struct EvaluationResult {
    pub outcome: AllowDenyOrApproval,
    pub reason_code: ReasonCode,
    pub policy_id: PolicyId,
    pub policy_version: u32,
    pub policy_hash: Hash,
    pub advisory_signals: Vec<AdvisorySignal>,
}
```

The deterministic result is the authorization source. Advisory signals are recorded separately and cannot silently alter the result.

## Evaluation order

```text
identity
→ workspace/environment/agent freeze
→ active policy and expiry
→ network and asset
→ merchant and payTo allowlist
→ amount limits
→ atomic budget reservation
→ velocity limits
→ approval threshold
```

The first blocking reason is stable and machine-readable. Internal diagnostics may include additional failed checks, but the external result must not depend on map iteration or nondeterministic ordering.

## Why deterministic enforcement

### Reproducibility

The same normalized input and policy version produce the same outcome, allowing incident replay and audit verification.

### Safety

Hard limits such as a maximum amount and an allowlist are exact constraints, not predictions.

### Availability

The core decision path does not depend on an external model provider, model quota, or network call.

### Explainability

Reason codes such as `daily_budget_exceeded` and `merchant_not_allowlisted` are directly tied to policy fields.

### Cost control

Every payment does not require an LLM call, keeping latency and operating cost predictable.

## LLM safety boundary

If an advisory model is introduced:

1. It receives redacted metadata, never private keys or bearer tokens.
2. It receives a bounded, canonical context.
3. Its output is schema-validated and size-limited.
4. Prompt content from a merchant or agent is treated as untrusted data.
5. A model failure is equivalent to “no advisory signal,” not an authorization failure or allow.
6. Model version and advisory output hash are stored for audit.
7. Human approval remains required for any risk-based escalation.

## Rejected alternatives

### LLM as the final policy engine

Rejected because probabilistic output, prompt injection, provider outages, and model drift are unacceptable for direct spending authority.

### LLM-only anomaly blocking

Rejected because it can produce false positives, false negatives, and non-reproducible decisions. It may supplement hard policy, not replace it.

### Client-side model enforcement

Rejected because a compromised agent can remove or alter the client model before signing.

## Consequences

Positive:

- exact budget and allowlist enforcement;
- deterministic audit and replay;
- predictable latency and cost;
- no model-provider dependency for safety-critical decisions;
- clear separation between policy and risk intelligence.

Negative:

- deterministic rules do not detect every behavioral anomaly;
- policy authors must maintain explicit rules;
- a future anomaly service adds operational complexity;
- unusual but legitimate payments may require human approval.

## Required tests

- repeated evaluation produces byte-equivalent decision output;
- evaluator never returns allow when a hard rule fails;
- adversarial merchant text cannot change policy behavior;
- advisory service timeout does not produce allow;
- advisory output cannot mutate budgets or policies;
- model version and signal hash are auditable when enabled.

## Revisit when

Revisit only if a formally bounded model-assisted evaluator can provide deterministic, independently verifiable authorization semantics. Until then, LLM output remains advisory.
