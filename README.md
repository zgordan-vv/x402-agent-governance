# x402 Agent Spend Governance

Non-custodial policy and audit control for autonomous x402 payments.

The service evaluates a payment intent before signing, enforces workspace and
agent policy, and records settlement evidence after signing. It never receives
or stores customer private keys.

## Try the hosted service

- API: <https://x402.codehummus.com>
- OpenAPI: <https://x402.codehummus.com/openapi.json>
- User manual: [docs/user-manual.md](docs/user-manual.md)

The hosted environment is intended for sandbox and devnet evaluation. Use the
public demo or a scoped agent credential; do not use production funds during
evaluation.

## Run locally

Requirements: Rust stable, Node.js 20+, pnpm 10+, Docker.

```bash
cp .env.example .env
pnpm install
cargo test --workspace
docker compose up --build
curl http://localhost:8080/health/ready
```

The local stack includes the API and PostgreSQL. The sandbox exercises the
same lifecycle without sending money:

```bash
pnpm --dir apps/sandbox test
PORT=8090 node apps/sandbox/src/server.mjs
```

## Core lifecycle

1. Evaluate a normalized x402 intent with an idempotency key.
2. Stop on `deny` or `approval_required`.
3. On `allow`, the customer-owned wallet signs the transaction.
4. Submit settlement evidence and preserve the resulting `settled`, `failed`,
   `unknown`, or `mismatch` state.

See the [user manual](docs/user-manual.md) for API examples, SDK integration,
devnet checks, monitoring, troubleshooting, and security rules.

## Safety

Never put private keys, seed phrases, bearer tokens, or production credentials
in `.env`, fixtures, logs, screenshots, or commits. See `docs/adr/` for the
architecture constraints used during implementation.
