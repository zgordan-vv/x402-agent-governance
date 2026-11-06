# Policy cookbook

Policies are immutable versions. Publish a new version instead of editing a live rule.

- **Allowlist:** restrict `network`, `asset`, merchant origin, recipient, and path prefix.
- **Caps:** set per-payment and rolling daily limits; reservations are atomic in PostgreSQL.
- **Approval:** require a human approval above a threshold; an approval is scoped to one intent.
- **Freeze:** freeze an agent, environment, or workspace to fail closed immediately.
- **Reconciliation:** record facilitator and chain evidence separately; mismatched amount or recipient is an incident, not success.

Test every policy with an allowed payment, a denied merchant, an over-cap payment, a frozen agent, and a duplicate idempotency key before publishing.
