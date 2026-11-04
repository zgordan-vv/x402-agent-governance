# ADR-002: Non-Custodial Architecture

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Guardrail controls autonomous x402 payments, but its value is policy enforcement and evidence—not custody of customer funds. Accepting seed phrases or private keys would turn the service into a custodian and dramatically increase the blast radius of a compromise.

## Decision

Guardrail never receives, stores, or signs with a customer wallet private key. The payment signer remains under customer control:

1. The SDK receives an x402 quote.
2. It sends a normalized intent without signing material.
3. Guardrail evaluates identity, policy, merchant, and budget.
4. An `allow` response includes a short-lived signed decision token.
5. The customer-owned signer signs locally or through the customer's own wallet service.
6. The SDK sends payment proof to the facilitator/merchant.
7. The SDK reports only public settlement evidence back to Guardrail.

Guardrail may manage one service private key for signing decision tokens. That key cannot authorize a blockchain transfer and is isolated from customer wallets.

## Trust boundary

```text
Customer application / signer
    owns the payment key
              |
              | intent and public settlement evidence
              v
Guardrail
    evaluates policy, reserves budget, signs decision token, stores audit
              |
              v
x402 facilitator / merchant
    validates proof and settles on-chain
```

An `allow` decision is not a settlement guarantee. Guardrail can block future payments, but cannot cancel a transaction already signed or settled on-chain. The UI must show policy decision, wallet signing, facilitator settlement, and on-chain observation as separate states.

## Decision-token binding

The token is authorization evidence, not a payment signature. It is bound to the intent hash, agent, network, asset, amount, payTo, policy version, expiry, and nonce. Changing any payment-critical field after evaluation invalidates the flow.

## API and security rules

- Accept public identities, merchant metadata, amounts, hashes, signatures, and settlement status.
- Reject `private_key`, `secret_key`, `seed_phrase`, `mnemonic`, and wallet-export fields.
- Never place signing material in logs, fixtures, exports, or telemetry.
- Return a customer credential only once at creation and store only its hash.
- Keep the decision signing key in a secret manager/KMS and rotate it independently of customer wallets.

## Rejected alternatives

Custodial hosted wallets, server-side signing, mandatory smart wallets, and client-only policy were rejected. They either expand the trust boundary, force wallet migration, or allow a compromised agent to bypass independent enforcement.

## Consequences

Positive: a Guardrail compromise cannot directly drain customer wallets; existing signers remain usable; self-hosting is simpler; responsibilities are clear.

Negative: customers must provide a signer; Guardrail cannot reverse an already signed transaction; ambiguous facilitator results require reconciliation; integration UX is more complex than a custodial wallet.

## Revisit when

Revisit only if research demonstrates demand for managed wallets. Any custody adapter must be opt-in and add KMS/HSM/MPC, consent, withdrawal controls, isolation, incident response, and legal review. The non-custodial core remains available independently.
