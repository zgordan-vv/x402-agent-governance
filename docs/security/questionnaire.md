# Security questionnaire

- Customer private keys: never collected or stored.
- Authorization: bearer credentials are scoped; tenant access is checked server-side.
- Decisions: fail closed; policy, reservation, and required audit write commit atomically.
- Replay: idempotency keys and short-lived signed decision tokens are verified.
- Secrets: environment/secret-manager only; logs redact credentials and payment payloads.
- Data: PostgreSQL is authoritative; audit evidence is append-only and hash chained.
- Recovery: encrypted backups, restore rehearsal, migration compatibility, and rollback runbook are required.
- External dependencies: RPC/facilitator observations are evidence, not policy authority.
