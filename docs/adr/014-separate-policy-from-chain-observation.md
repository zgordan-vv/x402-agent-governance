# ADR-014: Separate Policy Evaluation from Chain Observation

- **Status:** Accepted
- **Date:** 2026-11-05

## Context

Policy authorization must happen before a customer-owned signer is called. Blockchain RPC and facilitator APIs are external, rate-limited, sometimes slow, and occasionally inconsistent. Making an allow decision depend on an RPC lookup would add latency and create an availability dependency at the most sensitive point in the payment flow.

At the same time, a policy decision alone does not prove that a payment settled. The system needs a separate lifecycle for facilitator responses and on-chain observation.

## Decision

The authorization path is independent of external chain observation:

```text
quote → normalize → policy evaluation → budget reservation → decision token
                                                     |
                                             customer-owned signer
                                                     |
                                      facilitator / chain observation
                                                     |
                                             settlement evidence
```

The evaluator uses the payment intent supplied by the x402 quote and configured policy data. It does not call Solana/RPC or a facilitator to decide whether the payment is allowed.

After signing, the settlement service and optional observer adapters verify the payment result asynchronously or through an explicit settlement request. They may classify the result as `settled`, `failed`, `unknown`, or `mismatch` and reconcile the reservation.

## Why this separation

### Predictable authorization latency

Policy decisions remain bounded by local API and database performance instead of external RPC latency.

### Fail-closed control

If chain observation is unavailable, the system can still decide whether a future payment is allowed; if policy storage is unavailable, it cannot issue an allow token. The two failure modes have different safety meanings.

### Clear lifecycle semantics

`allow` means “the intent satisfied policy.” It does not mean “the transaction landed.” Separate states prevent a timeout from becoming a false success.

### Multi-chain and multi-facilitator support

Policy logic remains chain-neutral. Network-specific adapters implement settlement and observation for Solana, Base, or later networks.

## Adapter contract

An observation adapter receives public evidence and returns a normalized result:

```rust
pub enum ObservationResult {
    Settled { signature: String, observed_amount: AmountAtomic },
    Failed { code: String },
    Unknown { retry_after: Duration },
    Mismatch { field: String },
}
```

Adapters must not receive customer private keys. They use signatures, public addresses, facilitator receipts, and configured RPC credentials only.

## Rejected alternatives

### RPC lookup before authorization

Rejected because it makes policy availability depend on external chain infrastructure and adds no authorization fact that the quote/policy does not already contain.

### Treating facilitator acceptance as final settlement

Rejected because acceptance, propagation, confirmation, and final on-chain observation are distinct states.

### Chain-specific policy evaluator

Rejected because it would duplicate governance logic and make adding another network expensive.

### Synchronous observation before returning to the agent

Rejected as the default because it increases latency and can leave the caller unable to distinguish a safe policy result from a network timeout. Explicit clients may request a bounded confirmation mode later.

## Consequences

Positive:

- fast, deterministic policy decisions;
- smaller authorization failure surface;
- chain-neutral policy core;
- explicit settlement and reconciliation states;
- easier sandbox testing.

Negative:

- the agent may receive an allow before settlement is known;
- unknown reservations require background reconciliation;
- adapter implementations must handle chain-specific finality;
- UI must show policy and settlement as separate evidence.

## Required tests

- evaluator makes no external RPC calls;
- RPC outage does not change a valid local policy result;
- policy database outage never issues an allow token;
- facilitator acceptance is not automatically marked final;
- observer timeout produces `unknown` and a retryable job;
- amount/payTo/network mismatch produces a finding;
- multiple chain adapters map results to the same normalized state machine.

## Revisit when

Revisit if a protocol standard requires a verifiable external authorization check before every policy decision. Any such dependency must preserve bounded latency, fail-closed behavior, and separation between authorization and settlement evidence.
