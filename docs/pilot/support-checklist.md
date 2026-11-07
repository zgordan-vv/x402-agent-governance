# Pilot support checklist

Use this checklist for each pilot session.

## Before

- [ ] Participant has the manual and sandbox URL.
- [ ] Credential scope and expiry are recorded in the operator's secret store.
- [ ] Disposable wallet and network are confirmed.
- [ ] No production wallet or production RPC is configured.

## During

- [ ] Sandbox success is demonstrated.
- [ ] Deny path confirms the signer is not called.
- [ ] Mismatch and unknown outcomes remain non-success states.
- [ ] One real devnet signature is observed through the configured RPC.
- [ ] Participant can locate request, decision, and settlement identifiers.

## After

- [ ] Credential is revoked or expired.
- [ ] Temporary wallet is archived or discarded according to the pilot policy.
- [ ] Issues are recorded without secrets.
- [ ] Follow-up owner and due date are assigned.
