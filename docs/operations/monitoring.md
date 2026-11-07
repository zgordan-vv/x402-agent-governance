# MVP monitoring

The MVP reuses the Prometheus instance already running for Landfall on the shared DigitalOcean host. x402 exposes `/metrics` with the `governance_api_up` gauge. Prometheus scrapes it as job `x402-api` and alerts on scrape failure or a not-ready API.

The host already had port `9090` occupied by another service, so Prometheus is published locally on `127.0.0.1:9091`. To inspect it from a laptop:

```bash
ssh -N -L 9091:127.0.0.1:9091 admin.codehummus.com
```

Open <http://localhost:9091> and query:

```promql
up{job="x402-api"}
governance_api_up{job="x402-api"}
```

Both should be `1`. The current scrape target uses the x402 API container address on the shared Docker network; if the API container is recreated and its address changes, update the target and reload Prometheus. A follow-up should move this to a stable Docker network alias or service discovery.
