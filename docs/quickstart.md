# Sandbox quickstart (under ten minutes)

Requirements: Docker, Node 20+, pnpm 10+, and a checkout of this repository.

```bash
pnpm install
docker compose up -d postgres
pnpm --dir apps/sandbox test
PORT=8090 node apps/sandbox/src/server.mjs
```

In another terminal:

```bash
curl -s http://127.0.0.1:8090/health
curl -s -X POST http://127.0.0.1:8090/v1/merchant/quote \
  -H 'content-type: application/json' -d '{"amount_atomic":"10","pay_to":"merchant"}'
```

The sandbox uses a fake facilitator and no money. It demonstrates allow, deny, timeout, and mismatch outcomes through the same SDK lifecycle boundaries used by production adapters. Stop it with `Ctrl-C`, then `docker compose down`.
