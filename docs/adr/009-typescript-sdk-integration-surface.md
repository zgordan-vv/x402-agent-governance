# ADR-009: TypeScript SDK as the Primary Integration Surface

- **Status:** Accepted
- **Date:** 2026-11-04

## Context

Most initial x402 integrations are web, Node.js, or agent-tool workflows. The control layer must be easy to add around an existing fetch/payment client without forcing a wallet migration, framework adoption, or server-side custody.

The product also targets Rust teams, but Rust should not be the only path to adoption. A small TypeScript middleware can integrate with existing x402 clients and demonstrate value before a customer commits to a deeper platform change.

## Decision

TypeScript is the primary public SDK for MVP. It exposes a framework-neutral `GuardrailClient` and `governedFetch` middleware based on standard `fetch` and an injected signer interface.

The SDK owns:

- parsing x402 payment requirements;
- canonical intent construction;
- idempotency-key generation and persistence hooks;
- calling the evaluate endpoint;
- preventing signer invocation on deny;
- passing an approved decision token to the payment adapter;
- submitting settlement evidence;
- safe retry behavior for evaluation and settlement.

The SDK does not own:

- private keys;
- wallet custody;
- policy storage;
- final authorization rules;
- arbitrary retry of an uncertain signer operation.

## Integration shape

```ts
type Signer = (payment: PaymentRequest) => Promise<SignedPayment>;

const client = new GuardrailClient({
  baseUrl: process.env.GUARDRAIL_URL,
  credential: process.env.GUARDRAIL_AGENT_TOKEN,
});

const result = await client.governedFetch(url, {
  signer: customerOwnedSigner,
});
```

The signer is supplied by the customer application. The SDK never serializes or transmits its private material to Guardrail.

## Why TypeScript first

- broad compatibility with Node agent frameworks and API clients;
- no required runtime beyond Node.js 20 and fetch;
- simple local sandbox and test-merchant setup;
- low integration cost for customer discovery;
- natural fit for MCP and web service tooling;
- typed protocol can later be mirrored in Rust.

## Protocol rules

1. A 402 quote is not an authorization.
2. SDK evaluates before calling the signer.
3. `deny` and `policy_unavailable` stop the flow.
4. `approval_required` is returned to the caller unless an explicit approval adapter is configured.
5. Evaluation retries are safe with the same idempotency key.
6. A signer timeout is ambiguous and is not retried automatically.
7. Settlement retries are safe and idempotent.
8. SDK redacts credentials and payment secrets from errors and telemetry.

## Rust SDK

A Rust SDK is a planned second surface. It should consume the same canonical protocol types, reason codes, decision-token claims, and error semantics. It must not create a divergent policy implementation.

The Rust crate is released after the TypeScript contract and sandbox behavior stabilize.

## Rejected alternatives

### HTTP proxy only

Rejected as the only integration surface because many agents already have custom fetch/signing code and a transparent proxy can obscure signer boundaries.

### Rust SDK first

Rejected for MVP adoption because it excludes many agent and web teams from the first pilot, even though Rust remains the backend implementation language.

### Wallet-specific SDK

Rejected because it creates vendor lock-in and conflicts with the non-custodial architecture.

### Server-side signer integration

Rejected because it would expand custody and make the SDK responsible for customer funds.

## Consequences

Positive:

- small adapter surface;
- wallet/provider independence;
- easy sandbox demos;
- clear separation between policy and signing;
- common protocol types across future SDKs.

Negative:

- customers must implement or provide a signer;
- SDK must support multiple x402 response variants;
- TypeScript and Rust versions require compatibility tests;
- caller error handling must distinguish deny, outage, and ambiguous signing.

## Required tests

- allow path invokes the signer once;
- deny path never invokes the signer;
- policy outage never invokes the signer;
- repeated evaluation uses one idempotency key;
- malformed 402 is rejected safely;
- settlement retry does not duplicate the reservation;
- signer timeout is surfaced as ambiguous;
- credentials and payment secrets are absent from logs;
- TypeScript fixtures match the Rust canonicalization tests.

## Revisit when

Revisit if customer discovery shows that a different language or protocol surface is the dominant adoption path. Any replacement must preserve signer injection, non-custody, deterministic intent hashing, and safe retry semantics.
