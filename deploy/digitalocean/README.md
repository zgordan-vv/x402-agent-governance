# DigitalOcean infrastructure bootstrap

This directory provisions the host baseline for the Phase 15 Compose deployment. It deliberately does not create a droplet, configure DNS, or store credentials. Those operations require an explicitly selected DigitalOcean account/project and region.

## Recommended droplet

- Ubuntu 24.04 LTS
- 2 vCPU / 4 GB RAM for a pilot
- block storage or managed PostgreSQL for database data
- a reserved/static IPv4 address
- a separate staging droplet before production

Create the droplet with the provider console or `doctl`, attach an SSH key, and pass `cloud-init.sh` as user data. The script installs Docker Compose, creates `/opt/x402-agent-governance`, and allows only SSH/HTTP/HTTPS through UFW.

## Next handoff

1. Point a staging DNS name to the droplet IP.
2. Configure the TLS reverse proxy and managed PostgreSQL endpoint.
3. Inject `POSTGRES_PASSWORD`, `DECISION_KEY_ID`, and `DECISION_SIGNING_KEY_B64` through the deployment secret store.
4. Deploy with the production Compose overlay from `docs/deployment/production.md`.

Never put a DigitalOcean API token, database password, signing key, or SSH private key in this repository.
