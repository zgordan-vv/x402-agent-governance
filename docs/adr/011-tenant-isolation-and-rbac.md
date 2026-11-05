# ADR-011: Tenant-Scoped Isolation and Backend-Enforced RBAC

- **Status:** Accepted
- **Date:** 2026-11-05

## Context

Guardrail is a multi-tenant control plane. Workspaces contain agents, credentials, policies, merchants, budgets, decisions, settlements, and audit events. A broken authorization check could expose one customer's payment history or allow an operator to change another customer's spending policy.

The dashboard is not a security boundary. A malicious client can call the API directly, substitute object IDs, replay an old request, or modify a UI request. Tenant and role checks must therefore be enforced in the backend and supported by database constraints.

## Decision

Every customer-owned entity is scoped to a `workspace_id` and, where applicable, an `environment_id`. The authenticated request principal carries its workspace and role scope. Repository queries require the scope explicitly; handlers may not query by object ID alone.

Roles are enforced server-side:

- `owner` — workspace lifecycle, membership, credentials, policies, freezes, and exports;
- `security_admin` — policies, allowlists, freezes, credentials, and security settings;
- `operator` — agents and operational views, but no policy publication or membership changes;
- `approver` — approve/deny only eligible approval records;
- `auditor` — read-only decisions, settlements, and audit exports.

Every mutating action performs both tenant and role authorization before domain logic. Audit events record the acting principal and the target workspace.

## Request authorization flow

```text
bearer credential
  → hash lookup and revocation check
  → principal {workspace_id, environment scope, agent scope, roles}
  → route permission check
  → repository query with workspace predicate
  → domain operation
```

The API must reject a request when the workspace context is absent, ambiguous, or inconsistent with the credential. Cross-tenant IDs must produce the same safe not-found/forbidden behavior without revealing whether an object exists.

## Database safeguards

- Foreign keys connect agents, policies, intents, and audit rows to their workspace.
- Unique indexes include workspace scope where names or keys can repeat across tenants.
- Runtime roles cannot bypass repository constraints with arbitrary SQL.
- Optional PostgreSQL row-level security may be added for high-assurance deployments, but application authorization remains required.
- Audit events are append-only and include workspace identity.

## Credential scope

An agent credential is limited to one workspace, environment, agent, and set of scopes. A dashboard credential is never accepted by the ingestion/evaluation API unless its scopes explicitly allow that operation. Revocation and expiry are checked on every request or through a short, bounded cache.

## Rejected alternatives

### UI-only authorization

Rejected because users can call the API directly and browser controls can be bypassed.

### Global object IDs without tenant predicates

Rejected because an ID leak or substitution would make cross-tenant access easier to create and harder to audit.

### One shared administrator credential

Rejected because it removes attribution, prevents least privilege, and makes emergency revocation too broad.

### Separate database per tenant for MVP

Rejected because it adds provisioning, migration, backup, and connection-pool complexity before tenant scale is known. A database-per-tenant deployment can be offered later for high-isolation customers.

## Consequences

Positive:

- explicit tenant boundaries;
- least-privilege roles;
- actionable audit attribution;
- safer object-ID handling;
- clear path to database-level isolation enhancements.

Negative:

- every repository method needs scope parameters;
- RBAC tests increase implementation effort;
- support and break-glass workflows need carefully audited elevation;
- high-isolation customers may eventually require separate databases.

## Required tests

- every route is tested for each role;
- an object ID from another workspace cannot be read or modified;
- an agent credential cannot change policy or membership;
- an auditor cannot mutate state;
- an approver cannot approve outside its scope;
- revoked and expired credentials fail;
- workspace freeze blocks all new payment decisions;
- audit events identify actor and workspace;
- repository queries always apply tenant predicates.

## Revisit when

Revisit if customer requirements mandate database-per-tenant, regional data residency, or stronger cryptographic tenant isolation. Any alternative must preserve backend-enforced authorization and complete action attribution.
