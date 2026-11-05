# x402 Agent Spend Governance

Non-custodial policy and audit control for autonomous x402 payments.

## Phase 0 bootstrap

Requirements: Rust stable, Node.js 20+, pnpm 10+, Docker.

```bash
cp .env.example .env
cargo test --workspace
docker compose up --build
curl http://localhost:8080/health/ready
```

The current API exposes only liveness/readiness endpoints. Payment policy, PostgreSQL migrations, SDK middleware, and the dashboard arrive in later implementation phases. No private key or real payment is required for Phase 0.

## Safety

Never put private keys, seed phrases, bearer tokens, or production credentials in `.env`, fixtures, logs, or commits. See `docs/adr/` for the architecture constraints used during implementation.
