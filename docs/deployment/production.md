# Production deployment

The repository provides a production Compose overlay, but it is not a hosted service. Put a TLS reverse proxy (Caddy, nginx, or a cloud load balancer) in front of the API and dashboard, keep PostgreSQL on a private network, and provide secrets through the deployment platform rather than Git.

## Deploy

Create a deployment-only `.env` with strong values for `POSTGRES_PASSWORD`, `DECISION_KEY_ID`, and `DECISION_SIGNING_KEY_B64`. If another service already uses host port 8080, set `API_BIND_PORT=8082`. Run Compose with `--env-file .env` so the selected ports and secrets are explicit.

```bash
docker compose -f docker-compose.yml -f docker-compose.production.yml up -d --build
docker compose ps
curl -fsS http://127.0.0.1:8080/health
```

The production overlay binds the API to localhost, removes the PostgreSQL host port, restarts services, runs API/worker as non-root users, and gives them read-only filesystems with a small `/tmp`.

## TLS and network controls

The current MVP deployment uses the existing host reverse proxy at `https://x402.codehummus.com`, forwarding to the private API listener on `127.0.0.1:8082`. Terminate HTTPS at the reverse proxy, forward only `/health` and API routes needed by the dashboard, and restrict PostgreSQL to the Compose network. Do not expose port 5432 publicly. Use a firewall/security group to allow only HTTPS and administrative SSH from trusted addresses.

## Backups and restore rehearsal

```bash
POSTGRES_DB=governance POSTGRES_USER=governance \
  PGPASSWORD="$POSTGRES_PASSWORD" ./scripts/backup-postgres.sh
createdb governance_restore
pg_restore --clean --if-exists --no-owner --dbname=governance_restore backups/<dump-file>
```

Store encrypted backups outside the host and test restores before releases. Keep at least one off-site copy and document the retention period. A backup is not considered valid until a restore and health check succeed.

## Release and rollback

Build an immutable image tag from the Git commit, run migrations and smoke tests, then deploy. Keep the previous image tag available for rollback. Never roll back the database schema independently of the application migration compatibility window.
