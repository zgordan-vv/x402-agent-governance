# TypeScript SDK guide

Install the workspace package and construct `GuardrailClient` with the API URL, a short-lived agent credential, and an agent ID. The SDK parses an x402 `402` response, evaluates the normalized payment, and calls the customer-owned signer only after an `allow` decision.

```ts
const client = new GuardrailClient({
  baseUrl: process.env.GUARDRAIL_URL!,
  credential: process.env.GUARDRAIL_TOKEN!,
  agentId: "agent-demo",
});
const result = await client.governedFetch("https://merchant.example/pay", {
  signer: ({ decisionToken }) => wallet.sign(decisionToken),
});
```

Never pass private keys to Guardrail. A signer error is ambiguous and must not be retried automatically. Evaluation retries reuse the idempotency key; settlement evidence is submitted separately and a timeout is never treated as success.
