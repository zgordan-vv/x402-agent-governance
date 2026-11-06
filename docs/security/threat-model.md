# Security threat model

## Assets

- workspace credentials and role assignments;
- policy versions, budget reservations, decision tokens and settlement evidence;
- signing-key material and public-key metadata;
- tenant-scoped audit history.

## Trust boundaries

1. The browser/dashboard is untrusted and receives only a short-lived tab token.
2. Agents and customer signers are untrusted callers; they never receive governance private keys.
3. The API is the authorization boundary and PostgreSQL is the authoritative state boundary.
4. The worker is restartable and may process a job more than once.
5. Merchant/facilitator/chain observations are evidence, not policy authority.

## Threats and controls

| Threat | Control | Negative-path test |
| --- | --- | --- |
| Stolen credential | Argon2id hashes, expiry/revocation, RBAC, bounded bearer input | revoked/oversized credentials fail closed |
| Compromised agent | workspace/environment/agent scoping and policy evaluation | cross-tenant IDs return forbidden/not found |
| Prompt injection or merchant spoofing | merchant normalization, allowlists, no policy decisions from merchant text | malformed/userinfo/unsupported merchant rejected |
| Token replay/tampering | Ed25519 signature, exact claims, TTL, nonce table and atomic consume | modified fields, unknown key, expiry and replay fail |
| Race spending | locked budget counter and one transaction for intent/decision/reservation | concurrent reservation conflict |
| Database tampering | PostgreSQL constraints, immutable policy trigger, append-only audit trigger | mutation attempts fail |
| Key compromise/rotation | `kid`, public discovery, retained old public keys, secret injection | old token verifies only while retained |
| Oversized request/DoS | 256 KiB body limit, ten-second timeout, pool acquire timeout, credential length cap | oversized/slow requests fail safely |
| Secret leakage | no raw headers/bodies in tracing, SDK redaction, security headers | redaction tests and secret scan |

## Operational checks

Run `cargo audit`, `cargo deny check`, `pnpm audit`, and a repository secret scan before release. Production must inject `DECISION_SIGNING_KEY_B64` from a secret manager; it must never be committed to `.env`, logs, Docker layers, or exported reports.

## Residual risk

The dashboard is a browser client and must be served over HTTPS in production. The API does not custody customer wallet keys. Real chain/facilitator adapters and load/recovery testing are intentionally handled by later phases.
