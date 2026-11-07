# x402 Agent Governance — User Manual

## What the product does

x402 Agent Governance evaluates an agent's proposed payment before a wallet signs it. It applies allowlists, network and asset rules, per-payment limits, rolling budgets, approvals, and workspace freezes. After signing, it stores settlement evidence separately and distinguishes `settled`, `failed`, `unknown`, and `mismatch` outcomes.

The product never receives customer private keys. The customer-owned wallet signs transactions; x402 stores the decision and evidence needed to explain what happened.

## Choose a usage mode

### Public demo

Use the hosted demo at [x402.codehummus.com](https://x402.codehummus.com). The root page and [OpenAPI document](https://x402.codehummus.com/openapi.json) verify that the service is live. A complete dashboard demo should show prepared allow, deny, approval, freeze, settlement, mismatch, and unknown examples without requiring a customer token or real funds.

### Local sandbox

The sandbox uses the production SDK lifecycle with a fake merchant and facilitator. It never sends money.

```bash
pnpm install
pnpm --dir apps/sandbox test
PORT=8090 node apps/sandbox/src/server.mjs
```

In another terminal:

```bash
curl -s http://127.0.0.1:8090/health
curl -s -X POST http://127.0.0.1:8090/v1/merchant/quote \
  -H 'content-type: application/json' \
  -d '{"amount_atomic":"10","pay_to":"merchant"}'
```

Use the sandbox to test allow, deny, facilitator rejection, timeout, duplicate, and mismatch behavior before connecting a wallet or RPC.

### Hosted API integration

The API contract is available at `GET /openapi.json`. A protected client needs a scoped agent credential in the format issued by the workspace administrator. The credential must be sent as:

```http
Authorization: Bearer <credential-id>.<secret>
```

Never put this value in source control, browser local storage, screenshots, or support tickets.

## Payment lifecycle

1. Parse the merchant's x402 quote.
2. Normalize network, asset, amount, origin, path, and recipient.
3. Call `POST /v1/evaluate` with an idempotency key and request hash.
4. Stop immediately on `deny` or `approval_required`.
5. On `allow`, verify the short-lived decision token and call the customer-owned signer.
6. Submit the resulting transaction signature to `POST /v1/settlements`.
7. Record the facilitator/chain observation and display the final evidence state.

`allow` means only that policy accepted the intent. It does not mean that the blockchain transaction landed.

## Evaluate an intent

Example request:

```bash
curl -sS -X POST https://x402.codehummus.com/v1/evaluate \
  -H "authorization: Bearer $GUARDRAIL_TOKEN" \
  -H 'content-type: application/json' \
  -d '{
    "idempotency_key": "checkout-0001",
    "agent_id": "<agent-uuid>",
    "network": "solana-devnet",
    "asset": "SOL",
    "amount_atomic": "1000000",
    "merchant_origin": "https://merchant.example",
    "pay_to": "<recipient>",
    "request_hash": "sha256:<signed-request-hash>",
    "quote_expires_at": "2030-01-01T00:00:00Z"
  }'
```

Possible outcomes are `allow`, `deny`, and `approval_required`. Reuse the same idempotency key when safely retrying an evaluation.

## Submit settlement evidence

```bash
curl -sS -X POST https://x402.codehummus.com/v1/settlements \
  -H "authorization: Bearer $GUARDRAIL_TOKEN" \
  -H 'content-type: application/json' \
  -d '{
    "decision_id": "<decision-uuid>",
    "idempotency_key": "settlement-0001",
    "status": "settled",
    "transaction_signature": "<signature>",
    "observed_amount_atomic": "1000000",
    "observed_pay_to": "<recipient>",
    "evidence": {"source":"solana-devnet","slot":123}
  }'
```

The API returns `settled`, `failed`, `unknown`, or `mismatch`. A changed amount or recipient must remain `mismatch`; do not manually convert it to success.

## TypeScript SDK

The SDK handles quote parsing, evaluation retries, decision-token verification, customer-owned signing, and settlement submission. Configure it with the API URL, scoped credential, and agent ID. The signer callback must use a wallet controlled by your application.

```ts
const client = new GuardrailClient({
  baseUrl: "https://x402.codehummus.com",
  credential: process.env.GUARDRAIL_TOKEN!,
  agentId: process.env.GUARDRAIL_AGENT_ID!,
});

const result = await client.governedFetch(merchantUrl, {
  signer: ({ decisionToken }) => wallet.sign(decisionToken),
});
```

Do not automatically retry an ambiguous signing operation. Evaluation and settlement retries are safe only when their idempotency keys are preserved.

## Solana devnet test

Use a disposable wallet and tiny amounts. Verify the public address first:

```bash
export DEVNET_WALLET_ADDRESS=<public-address>
export SOLANA_RPC_URL=https://api.devnet.solana.com
pnpm devnet:preflight
```

After sending a transaction, verify its signature:

```bash
export DEVNET_SIGNATURE=<signature>
pnpm devnet:preflight
```

The command reports whether the signature is visible, its slot, execution error, and an Explorer URL. A missing transaction is `unknown`, never success.

## Dashboard and monitoring

The hosted API health endpoints are `/health/live` and `/health/ready`. The MVP Prometheus UI is protected at [monitoring.codehummus.com](https://monitoring.codehummus.com). The x402 target should report `up = 1` and `governance_api_up = 1`.

## Troubleshooting

- `401`: credential is missing, malformed, expired, or revoked.
- `403`: credential is valid but lacks the agent/workspace scope.
- `409`: duplicate, replay, budget conflict, or invalid lifecycle transition.
- `422`: quote or request validation failed; check expiration and normalized fields.
- `503`: service, database, or signing configuration is unavailable.
- `unknown`: external facilitator/RPC evidence is incomplete; investigate or replay durable work, but do not mark it successful.

Always keep the `x-request-id`, decision ID, intent ID, and settlement ID when contacting support.

## Security rules

- Never send seed phrases or private keys to x402.
- Use separate credentials per agent and environment.
- Use devnet/sandbox before mainnet or production funds.
- Rotate and revoke credentials after demos or pilots.
- Keep PostgreSQL private and use HTTPS for hosted access.
- Treat audit exports and transaction signatures as sensitive operational evidence.
