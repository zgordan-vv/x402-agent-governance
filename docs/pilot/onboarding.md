# Pilot onboarding

This runbook is for a controlled sandbox/devnet pilot. It is not a production
funds onboarding procedure.

## 1. Before the session

The operator prepares:

- one workspace and one scoped agent credential per pilot participant;
- an agent ID and the API base URL;
- a disposable devnet wallet owned by the participant;
- a merchant endpoint or the local sandbox demo;
- a support contact and an agreed test window.

Never request a seed phrase, private key, wallet export, or unrestricted bearer
credential. The participant keeps signing in their own wallet.

## 2. Participant checklist

1. Open the [user manual](../user-manual.md).
2. Run `pnpm demo:sandbox` or open the sandbox browser demo.
3. Confirm that `deny`, `mismatch`, and `unknown` are not displayed as success.
4. Configure the SDK with the scoped credential supplied for this pilot.
5. Run one tiny devnet payment from the disposable wallet.
6. Record the decision ID, settlement ID, transaction signature, and outcome.
7. Revoke the pilot credential and stop using the disposable wallet after the
   session.

## 3. Success criteria

The pilot is successful when the participant can explain:

- why a payment was allowed or denied;
- why signing remains in the participant's wallet;
- why an allow decision is not proof of settlement;
- how `unknown` and `mismatch` require investigation;
- where to find the audit evidence and request ID.

## 4. Escalation

When reporting a problem, include only non-secret identifiers:

- UTC timestamp;
- request ID;
- decision ID and settlement ID;
- HTTP status and reason code;
- public transaction signature, if any;
- environment and network.

Do not attach bearer tokens, payment headers, seed phrases, private keys, or
full customer payloads.
