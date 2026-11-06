# Incident response

1. Freeze the affected workspace or agent.
2. Revoke the credential and rotate signing keys if compromise is suspected.
3. Preserve request IDs, decision IDs, signatures, and audit exports; do not copy secrets into tickets.
4. Determine whether the event is policy denial, duplicate, mismatch, facilitator timeout, or chain unknown.
5. Reconcile only from durable evidence. Never convert an unknown result to success manually.
6. Restore service, replay safe durable jobs, and document the timeline and corrective action.
