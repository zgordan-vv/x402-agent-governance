# API reference

The canonical machine-readable contract is served by `GET /openapi.json`. All protected routes require a bearer credential with the documented scope.

| Route | Purpose |
|---|---|
| `GET /health/live` | process liveness |
| `GET /health/ready` | database/readiness check |
| `POST /v1/evaluate` | evaluate and reserve an intent |
| `POST /v1/settlements` | submit settlement evidence |
| `POST /v1/decision-tokens/consume` | consume a signed decision token |
| `GET /v1/audit/events` | read audit evidence |
| `GET /v1/exports` | export evidence |
| `GET /v1/policies` | list policy versions |
| `POST /v1/agents` | manage agents |
| `POST /v1/approvals` | approve or reject pending intents |
| `POST /v1/workspace/freeze` | freeze/unfreeze a workspace |

Unknown settlement outcomes remain unknown. Clients should handle `400`, `401`, `403`, `409`, `422`, and `503` explicitly and preserve the returned request ID for support.
