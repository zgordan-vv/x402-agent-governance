# System Design: x402 Agent Spend Governance

Версия: 1.0  
Статус: design baseline для MVP  
Связанный документ: [`prd.md`](./prd.md)

## 1. Назначение системы

Система контролирует автономные платежи AI-агентов перед подписью транзакции. Она не хранит приватные ключи и не является wallet custodian. Клиентский SDK получает x402 quote, отправляет payment intent в Guardrail, получает детерминированное решение и только после `allow` передаёт управление customer-owned signer.

Главный инвариант:

> Ни один платёж не должен быть подписан автоматически, если он не соответствует активной policy, identity агента, merchant allowlist и оставшемуся бюджету.

После подписи система принимает settlement evidence и связывает его с исходным decision. Разрешение и фактический on-chain результат — разные состояния и не должны смешиваться.

## 2. Functional Requirements

### FR-1. Tenant и environment isolation

- Система поддерживает несколько workspaces.
- Каждый workspace имеет sandbox, devnet и production environments.
- Все agents, policies, budgets, intents и audit events принадлежат одному workspace/environment.
- Запрос не может читать или изменять объект другого workspace.

### FR-2. Agent identity

- Каждый agent получает стабильный `agent_id`.
- Credentials имеют scope, expiry, статус и hash вместо plaintext secret.
- Credential можно отозвать без удаления agent.
- Agent не может сам изменить собственные policy или budgets.

### FR-3. Merchant identity

- Merchant нормализуется из endpoint origin и `pay_to`.
- Allowlist может ограничивать origin, host, path prefix, network и payTo.
- Неизвестный merchant блокируется при default-deny policy.
- Изменение allowlist создаёт новую policy version.

### FR-4. Policy evaluation

Evaluator проверяет:

1. credential и agent status;
2. workspace/environment freeze;
3. active policy и её effective/expiry time;
4. network и asset;
5. merchant и payTo allowlist;
6. per-payment limit;
7. session, daily и monthly budgets;
8. velocity rule;
9. approval threshold;
10. idempotency/replay constraints.

Результат только один из: `allow`, `deny`, `approval_required`.

### FR-5. Atomic budgets

- Budget reservation и создание decision выполняются в одной транзакции.
- Параллельные requests не могут совместно превысить лимит.
- Reservation имеет TTL и lifecycle `reserved → settled`, `reserved → released` или `reserved → unknown`.
- Один idempotency key не создаёт второй расход.

### FR-6. Decision token

Для `allow` система возвращает короткоживущий подписанный token.

Token привязан к:

- intent hash;
- agent и environment;
- merchant и payTo;
- network и asset;
- approved amount;
- policy version;
- expiry и nonce.

### FR-7. Human approval

- Сумма выше approval threshold создаёт pending approval.
- Approver может approve или deny только в рамках своего scope.
- Approval имеет TTL.
- Approved decision нельзя расширить по сумме, merchant или времени.

### FR-8. Settlement evidence

- SDK отправляет transaction signature/hash и facilitator result.
- Система сверяет settlement с approved intent.
- Mismatch создаёт finding и не считается успешным settlement.
- Timeout и неизвестный результат сохраняются как `unknown`, а не как success.

### FR-9. Emergency controls

- Freeze на workspace, environment, agent и policy.
- Freeze действует fail-closed.
- Новые intents получают стабильный reason code.
- Unfreeze требует role `owner` или `security_admin` и оставляет audit event.

### FR-10. Audit и exports

- Все security-relevant действия immutable.
- Audit event содержит actor, action, target, policy version, reason и correlation ID.
- Доступны фильтрация, pagination и JSON/CSV export.
- Secrets, private keys и полные платёжные payloads не попадают в audit.

### FR-11. Dashboard

Dashboard предоставляет:

- создание workspace и environment;
- регистрацию agents;
- policy editor и decision preview;
- merchant allowlist;
- budget usage;
- approval queue;
- freeze/unfreeze;
- audit explorer;
- sandbox walkthrough.

## 3. Non-Functional Requirements

### NFR-1. Security

- Default deny при отсутствии policy или при ошибке evaluator.
- TLS для всех внешних API.
- Credentials хранятся в виде Argon2id/SHA-256 hash с rotation metadata.
- RBAC enforced на backend, не только в UI.
- Decision tokens подписываются Ed25519 ключом service key.
- Private keys агента никогда не принимаются сервером.
- Structured logs проходят secret redaction.
- Все mutating requests поддерживают idempotency.
- Rate limit отдельно для evaluation, settlement и audit APIs.

### NFR-2. Consistency

- Policy version immutable после публикации.
- Budget counters изменяются атомарно в PostgreSQL.
- Один intent имеет один окончательный decision.
- Audit event sequence монотонен в пределах aggregate.

### NFR-3. Availability

- Целевой MVP uptime: 99.5%.
- При недоступности policy или database платежи не разрешаются автоматически.
- Settlement endpoint допускает повтор после восстановления.
- Dashboard может быть временно недоступен без изменения payment safety.

### NFR-4. Performance

- Policy evaluation p95 < 100 ms без внешнего RPC.
- Policy evaluation p99 < 300 ms.
- Settlement acknowledgement p95 < 150 ms.
- Audit list p95 < 500 ms для первой страницы.
- Один production instance: 100 decisions/sec и 500 audit events/sec.

### NFR-5. Privacy

- Не хранить paid response body.
- Request content represented by hash и минимальными labels.
- Configurable retention для audit и settlement metadata.
- Экспорт redacts credentials и internal signing metadata.

### NFR-6. Operability

- Health/readiness endpoints.
- Metrics: decision latency, deny rate, reservation conflicts, settlement mismatch, queue lag.
- Structured correlation IDs.
- Backup и restore rehearsal.
- Alerting на freeze spikes, budget contention и audit write failures.

## 4. Capacity Estimation

### 4.1 Assumptions для MVP

| Параметр | Значение |
|---|---:|
| Workspaces | 1,000 |
| Agents/workspace | 20 |
| Agents всего | 20,000 |
| Среднее intents/agent/day | 500 |
| Intents/day | 10,000,000 |
| Peak multiplier | 10x среднего |
| Peak evaluation rate | ~1,200/sec |
| Audit events/intents | 2.5 |
| Retention | 90 дней |

Это sizing target, а не обещание текущей реализации. MVP можно запустить с одной PostgreSQL-инстанцией на существенно меньшей нагрузке.

### 4.2 Storage

Средняя запись intent + decision + audit metadata оценивается в 2–4 KB до индексов.

```text
10,000,000 intents/day × 3 KB ≈ 30 GB/day raw
90 days ≈ 2.7 TB raw
indexes + WAL + replicas ≈ 2–3x
recommended durable storage ≈ 8 TB for this target
```

Для pilot на 100,000 intents/day:

```text
100,000 × 3 KB ≈ 300 MB/day
90 days ≈ 27 GB raw
recommended volume ≈ 100–150 GB
```

### 4.3 Database strategy

- MVP: PostgreSQL 16, connection pool, composite tenant indexes.
- Pilot: monthly partitioning для `audit_events`, `payment_intents` и `settlements`.
- Production scale: read replica для audit queries и object storage для cold exports.
- Counters остаются в hot tables, а старые audit partitions архивируются отдельно.

### 4.4 Caching

Policy и public signing-key metadata можно кэшировать на 30–60 секунд.

Нельзя кэшировать budget decision без атомарной reservation в authoritative database.

## 5. API Design

### 5.1 Общие правила

- Base URL: `/v1`.
- JSON UTF-8.
- UUIDv7 для sortable IDs.
- `Authorization: Bearer <credential>`.
- `X-Request-Id` принимается или создаётся сервером.
- `Idempotency-Key` обязателен для intent и settlement writes.
- Ошибка имеет `code`, `message`, `retryable`, `request_id`.

### 5.2 Evaluate intent

`POST /v1/intents/evaluate`

Request:

```json
{
  "idempotency_key": "idem_01J...",
  "agent_id": "agent_researcher",
  "environment_id": "env_devnet",
  "merchant": {
    "origin": "https://api.example.com",
    "path": "/v1/data",
    "pay_to": "merchant-public-key"
  },
  "payment": {
    "network": "solana-devnet",
    "asset": "USDC",
    "amount_atomic": "250000",
    "decimals": 6
  },
  "quote_expires_at": "2026-11-04T12:00:00Z",
  "request_hash": "sha256:..."
}
```

Response `200`:

```json
{
  "decision": "allow",
  "decision_id": "dec_01J...",
  "intent_id": "int_01J...",
  "policy_id": "pol_01J...",
  "policy_version": 12,
  "reason_code": "within_budget_and_allowlist",
  "budget": {
    "daily_remaining_atomic": "9750000",
    "reservation_atomic": "250000"
  },
  "expires_at": "2026-11-04T12:00:30Z",
  "decision_token": "signed-token"
}
```

`deny` и `approval_required` используют тот же envelope, но без usable decision token.

### 5.3 Submit settlement

`POST /v1/intents/{intent_id}/settlement`

```json
{
  "idempotency_key": "settle_01J...",
  "decision_id": "dec_01J...",
  "transaction_signature": "base58-or-chain-hash",
  "facilitator_status": "settled",
  "observed_amount_atomic": "250000",
  "observed_pay_to": "merchant-public-key",
  "observed_at": "2026-11-04T12:00:08Z"
}
```

Possible outcomes: `settled`, `failed`, `unknown`, `mismatch`, `duplicate`.

### 5.4 Policy endpoints

```text
POST /v1/policies
GET  /v1/policies
GET  /v1/policies/{policy_id}
POST /v1/policies/{policy_id}/publish
POST /v1/policies/{policy_id}/freeze
```

Policy publish создаёт новую immutable version; update существующей опубликованной версии запрещён.

### 5.5 Agent endpoints

```text
POST /v1/agents
GET  /v1/agents
POST /v1/agents/{agent_id}/credentials
POST /v1/agents/{agent_id}/revoke
POST /v1/agents/{agent_id}/freeze
```

Plaintext credential возвращается только один раз при создании.

### 5.6 Approval endpoints

```text
GET  /v1/approvals?status=pending
POST /v1/approvals/{approval_id}/approve
POST /v1/approvals/{approval_id}/deny
```

Approver request содержит optimistic concurrency version, чтобы два approver не выдали противоречивые решения.

### 5.7 Audit endpoints

```text
GET  /v1/audit/events
GET  /v1/audit/events/{event_id}
POST /v1/audit/exports
GET  /v1/audit/exports/{export_id}
```

Параметры: `agent_id`, `merchant`, `decision`, `reason_code`, `from`, `to`, `cursor`, `limit`.

## 6. High-Level Design

```text
                         +----------------------+
                         | Dashboard             |
                         | policies/approvals   |
                         +----------+-----------+
                                    |
                          HTTPS / REST API
                                    |
+------------+       +-------------v-------------+
| AI agent   |       | API Gateway               |
| x402 SDK   +-------> auth, validation, limits  |
+-----+------+       +-------------+-------------+
      |                              |
      |                        +-----v------+
      |                        | Evaluator  |
      |                        | policy     |
      |                        +--+------+--+
      |                           |      |
      |                     +-----v+    +v------+
      |                     |Budget|    |Approval|
      |                     +--+---+    +---+----+
      |                        |            |
      |                        +------+-----+
      |                               |
      |                       +-------v--------+
      |                       | PostgreSQL     |
      |                       | control/audit |
      |                       +-------+--------+
      |                               |
      |   allow + decision token      |
      +-------------------------------+
      |
      v
customer-owned signer
      |
      v
x402 facilitator / merchant
      |
      +---- settlement evidence ----> Settlement API -> Audit
```

### Request path

1. Gateway authenticates credential.
2. Request validator normalizes quote and merchant.
3. Evaluator loads active policy version.
4. Budget service performs atomic reservation.
5. Decision service persists immutable decision.
6. Token signer issues short-lived token for `allow`.
7. SDK invokes customer-owned signer only after allow.
8. Facilitator settles payment.
9. SDK posts settlement evidence.
10. Settlement service finalizes/release/marks unknown reservation.

### Failure path

Если evaluator, database или policy store недоступны, gateway возвращает `503 policy_unavailable` и не выдаёт decision token. Это намеренно fail-closed.

## 7. Database Design

### 7.1 Schemas

```text
control   — tenants, users, agents, credentials, environments
policy    — policies, versions, rules, merchants
payments  — intents, decisions, reservations, approvals, settlements
audit     — append-only evidence and hash-chain metadata
```

### 7.2 Core tables

#### `control.workspaces`

```text
workspace_id uuid primary key
name text not null
status text check (status in ('active','frozen','deleted'))
created_at timestamptz not null
```

#### `control.agents`

```text
agent_id uuid primary key
workspace_id uuid not null references control.workspaces
environment_id uuid not null references control.environments
name text not null
status text not null
metadata jsonb not null default '{}'
created_at timestamptz not null
unique (workspace_id, environment_id, name)
```

#### `control.credentials`

```text
credential_id uuid primary key
agent_id uuid not null references control.agents
secret_hash bytea not null
scopes text[] not null
expires_at timestamptz
revoked_at timestamptz
created_at timestamptz not null
```

#### `policy.policy_versions`

```text
policy_id uuid not null
version integer not null
workspace_id uuid not null
environment_id uuid not null
status text check (status in ('draft','active','superseded','frozen'))
rules jsonb not null
created_by uuid not null
created_at timestamptz not null
published_at timestamptz
primary key (policy_id, version)
```

`rules` проходит schema validation до publish. Published rows immutable.

#### `policy.merchants`

```text
merchant_id uuid primary key
workspace_id uuid not null
origin text not null
path_prefix text
pay_to text
network text
asset text
status text not null
unique (workspace_id, origin, path_prefix, pay_to, network, asset)
```

#### `payments.payment_intents`

```text
intent_id uuid primary key
workspace_id uuid not null
environment_id uuid not null
agent_id uuid not null
idempotency_key text not null
intent_hash bytea not null
merchant jsonb not null
amount_atomic numeric(78,0) not null
asset text not null
network text not null
status text not null
created_at timestamptz not null
expires_at timestamptz not null
unique (workspace_id, agent_id, idempotency_key)
```

#### `payments.decisions`

```text
decision_id uuid primary key
intent_id uuid unique not null
policy_id uuid not null
policy_version integer not null
outcome text not null
reason_code text not null
explanation text not null
token_key_id text
token_hash bytea
expires_at timestamptz
created_at timestamptz not null
```

#### `payments.budget_reservations`

```text
reservation_id uuid primary key
decision_id uuid unique not null
budget_window text not null
amount_atomic numeric(78,0) not null
status text not null
expires_at timestamptz not null
finalized_at timestamptz
```

#### `payments.settlements`

```text
settlement_id uuid primary key
intent_id uuid not null
decision_id uuid not null
transaction_signature text
facilitator_status text not null
observed_amount_atomic numeric(78,0)
observed_pay_to text
outcome text not null
evidence jsonb not null default '{}'
created_at timestamptz not null
unique (decision_id, transaction_signature)
```

#### `audit.events`

```text
event_id uuid primary key
workspace_id uuid not null
aggregate_type text not null
aggregate_id uuid not null
sequence bigint not null
event_type text not null
actor_type text not null
actor_id text not null
payload jsonb not null
prev_hash bytea
event_hash bytea not null
occurred_at timestamptz not null
unique (aggregate_id, sequence)
```

### 7.3 Critical transaction: evaluate and reserve

```sql
BEGIN;

-- 1. Lock the relevant budget row/window.
SELECT * FROM payments.budget_counters
WHERE agent_id = $1 AND window_start = $2
FOR UPDATE;

-- 2. Check remaining amount and velocity constraints.
-- 3. Insert intent and immutable decision.
-- 4. Increment reserved amount.
-- 5. Insert reservation and audit event.

COMMIT;
```

Если любой check fails, транзакция откатывается целиком: не должно существовать decision без корректного budget state.

### 7.4 Indexes

```text
payment_intents(workspace_id, created_at desc)
payment_intents(agent_id, created_at desc)
payment_intents(workspace_id, idempotency_key)
decisions(workspace_id, outcome, created_at desc)
settlements(decision_id, created_at desc)
audit_events(workspace_id, occurred_at desc)
audit_events(aggregate_id, sequence)
```

## 8. Detailed Components Design

### 8.1 TypeScript/Rust SDK middleware

Responsibilities:

- parse x402 response;
- normalize quote;
- generate idempotency key;
- call evaluate endpoint;
- stop before signer on deny;
- pass decision token to payment client;
- submit settlement evidence;
- retry only idempotent operations.

SDK не принимает private key в Guardrail API. Signer interface остаётся локальным:

```ts
type Signer = (payment: PaymentRequest) => Promise<SignedPayment>;

async function governedPay(quote: X402Quote, signer: Signer) {
  const decision = await guardrail.evaluate(toIntent(quote));
  if (decision.decision !== "allow") return decision;

  const signed = await signer(quote.payment);
  return guardrail.settle(decision.intent_id, {
    decision_id: decision.decision_id,
    transaction_signature: signed.signature,
  });
}
```

### 8.2 API Gateway

Обязанности:

- TLS termination;
- bearer credential extraction;
- hash lookup и revocation check;
- request size limits;
- rate limiting;
- correlation ID;
- schema validation;
- mapping domain errors to HTTP statuses.

Gateway не принимает policy decisions сам и не содержит budget logic.

### 8.3 Policy repository

Хранит draft и published versions. Publish выполняет:

1. JSON schema validation;
2. semantic validation (например, daily cap не меньше per-call cap);
3. duplicate rule detection;
4. immutable version allocation;
5. audit event.

### 8.4 Policy evaluator

Evaluator получает уже нормализованный `EvaluationContext` и возвращает `DecisionResult`.

Порядок проверок фиксирован, чтобы reason был детерминированным:

```text
identity → freeze → policy validity → network/asset
→ merchant → amount → budget → velocity → approval
```

Первый blocking reason возвращается клиенту, остальные могут сохраняться как internal diagnostics.

### 8.5 Budget service

Budget service отвечает за windows:

- `session`: от создания session до session expiry;
- `daily`: UTC day;
- `monthly`: UTC calendar month.

Для каждой window выполняется `SELECT ... FOR UPDATE`. В production можно перейти на advisory locks или sharded counter rows, сохранив PostgreSQL как source of truth.

### 8.6 Approval service

Approval record хранит requested amount, policy version, requester, eligible approver scope и expiry. Approve создаёт decision только если:

- approval ещё pending;
- policy version всё ещё active;
- intent не expired;
- agent/workspace не frozen;
- amount и merchant не изменились.

### 8.7 Decision signer

Signer держит только service private key для подписания decision tokens, не wallet funds. Key ID и public key публикуются через JWKS-like endpoint. Rotation создаёт новый key ID; старые keys остаются доступными до истечения всех tokens.

### 8.8 Settlement service

Settlement service сравнивает:

```text
approved amount == observed amount
approved payTo == observed payTo
approved network == reported network
decision token == intent hash
```

При несовпадении reservation не финализируется как success и создаётся `settlement_mismatch` finding.

### 8.9 Audit writer

Audit writer добавляет hash chain:

```text
event_hash = SHA-256(prev_hash || canonical_payload || metadata)
```

Hash chain обеспечивает обнаружение изменения, но не заменяет backup, database ACL и external anchoring. Внешний anchor — post-MVP опция.

### 8.10 Dashboard frontend

Frontend не является security boundary. Все permissions повторно проверяются API.

Основные экраны:

- Overview: spend, denies, pending approvals, recent findings;
- Agents: identity, status, credentials, freeze;
- Policies: draft, diff, publish, version history;
- Merchants: allowlist;
- Approvals: approve/deny с reason;
- Audit: filters, event detail, export;
- Sandbox: безопасное воспроизведение сценариев без денег.

### 8.11 Background jobs

Нужны durable jobs для:

- истечения reservations;
- перевода expired approvals в deny;
- budget window rollups;
- audit export generation;
- retention/archive;
- settlement reconciliation.

Job execution должна быть idempotent и иметь lease/retry/dead-letter semantics.

### 8.12 Observability

Metrics:

```text
guardrail_decisions_total{outcome,reason_code}
guardrail_evaluation_duration_ms
guardrail_budget_conflicts_total
guardrail_settlement_mismatch_total
guardrail_approval_age_seconds
guardrail_audit_write_failures_total
guardrail_frozen_agents
```

Logs должны включать `request_id`, `workspace_id`, `agent_id`, `intent_id`, `decision_id`, но не credentials или raw headers.

## 9. Deployment Design

### MVP local

```text
Docker Compose:
  api
  worker
  dashboard
  postgres
```

### Pilot cloud

- managed PostgreSQL;
- one API service;
- one worker service;
- static dashboard hosting;
- secrets manager;
- TLS reverse proxy;
- daily backups;
- metrics and alerting.

### Production evolution

- separate API and evaluator autoscaling;
- read replica for audit;
- partitioned event tables;
- object storage exports;
- external KMS for decision signing key;
- regional failover.

## 10. Testing Strategy

### Unit tests

- every reason code;
- rule precedence;
- amount/decimals conversion;
- time windows;
- canonical intent hashing;
- token expiry;
- merchant normalization.

### Property tests

- evaluator never returns allow when any blocking rule fails;
- duplicate idempotency key has one reservation;
- concurrent reservations never exceed cap;
- changing irrelevant metadata does not change intent hash.

### Integration tests

- API + PostgreSQL transaction;
- approval lifecycle;
- freeze propagation;
- settlement mismatch;
- credential revocation;
- audit pagination/export.

### Security tests

- replay old decision token;
- alter amount/payTo after approval;
- concurrent double spend;
- cross-tenant object ID access;
- malformed quote;
- prompt injection cannot alter policy;
- secrets absent from logs and exports.

### End-to-end sandbox

1. Fake merchant returns x402 quote.
2. SDK asks Guardrail.
3. Policy allows test amount.
4. Fake signer produces test proof.
5. Settlement is recorded.
6. Dashboard shows decision and evidence.

Реальные devnet payments добавляются только после успешного sandbox suite.

## 11. Design Decisions

### PostgreSQL как source of truth

Выбран PostgreSQL, потому что evaluation decision, budget reservation и audit event требуют транзакционной согласованности. Redis может использоваться как cache/rate limiter, но не как единственный ledger.

### Off-chain policy enforcement в MVP

Это позволяет быстро интегрироваться с существующими x402 clients и не требовать migration средств в smart wallet. Более сильное on-chain enforcement может быть добавлено как отдельный adapter.

### Fail closed

Если policy service не может доказать, что платёж разрешён, он не выдаёт decision token. Это предпочтительнее автоматической оплаты при временной ошибке контроля.

## 12. Open Technical Questions

1. Нужен ли отдельный Rust evaluator library для embedded/self-hosted mode?
2. Какой canonical representation использовать для разных x402 versions?
3. Должен ли budget reservation освобождаться после facilitator timeout автоматически или требовать reconciliation?
4. Какой уровень on-chain verification нужен для разных networks?
5. Где хранить external audit anchors и какова их стоимость?
6. Какие поля merchant identity считаются устойчивыми при redirects и CDN?

