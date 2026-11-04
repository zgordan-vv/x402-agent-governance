# PRD: x402 Agent Spend Governance

## 1. Краткое описание

### Рабочее название

**Guardrail** — policy и audit layer для автономных x402-платежей.

### Проблема

x402 стандартизирует запрос цены, оплату и повторный HTTP-запрос с proof of payment, но не определяет, имеет ли конкретный агент право совершать платёж. Без внешней политики агент может оплатить неизвестный merchant, превысить бюджет, повторить платёж, потратить средства после компрометации или оставить команду без объяснимого аудита.

### Решение

Guardrail — non-custodial policy gateway и SDK, который получает платёжное намерение, проверяет его по versioned policy, возвращает allow/deny/approval decision до подписи, обеспечивает idempotency и сохраняет tamper-evident audit evidence.

MVP не хранит приватные ключи и не подписывает транзакции на сервере. Клиентский агент или customer-owned wallet остаётся владельцем ключа.

## 2. Цели продукта

1. Не допускать платежи, не соответствующие policy.
2. Делать каждое решение объяснимым и проверяемым.
3. Подключаться к существующему x402 client через минимальные изменения.
4. Поддерживать multi-agent, multi-merchant и multi-environment workflows.
5. Обеспечить безопасный sandbox/devnet путь для тестирования.
6. Дать security/platform-команде kill switch и audit export.

## 3. Не входит в MVP

- custody пользовательских средств;
- хранение seed phrases или private keys;
- самостоятельный facilitator;
- универсальный wallet;
- автоматическое возмещение on-chain платежа;
- юридическое разрешение споров;
- LLM, принимающая финальное решение вместо deterministic policy;
- полноценный billing и marketplace.

LLM может появиться позже как advisory anomaly detector, но никогда не должна быть единственным enforcement mechanism.

## 4. Пользователи и роли

### Workspace owner

Создаёт workspace, environments и policies, управляет участниками и аварийной остановкой.

### Security/platform administrator

Настраивает allowlists, caps, approval thresholds, key rotation и audit retention.

### Agent operator

Подключает SDK, получает decision и обрабатывает allow/deny/approval response.

### Approver

Разрешает исключительные платежи в рамках delegation scope.

### Auditor

Читает decision и payment evidence, экспортирует отчёт, но не меняет policy.

### Merchant/API owner

Публикует endpoint identity и получает предсказуемые payment requests; в MVP не получает доступ к customer policy.

## 5. Основные пользовательские сценарии

### UC-1: Разрешённый платёж

1. Агент получает x402 quote.
2. SDK извлекает merchant, network, asset, amount, endpoint и request identity.
3. SDK отправляет intent в Guardrail.
4. Policy engine проверяет scope агента, allowlist и budgets.
5. Guardrail возвращает `allow` с decision ID и policy version.
6. Агент подписывает платёж своим ключом.
7. Агент отправляет payment proof merchant/facilitator.
8. SDK отправляет settlement result в audit endpoint.

### UC-2: Отклонение по лимиту

Если сумма выше per-payment или оставшегося daily budget, Guardrail возвращает `deny` с machine-readable reason. Агент не должен вызывать wallet signer.

### UC-3: Отклонение по merchant

Если endpoint или payTo не входит в allowlist, решение `deny`, даже если сумма находится в пределах бюджета.

### UC-4: Approval required

Для платежа выше автоматического порога создаётся pending decision. Approver разрешает или отклоняет его в dashboard/API. Decision имеет TTL и не может быть использован после истечения.

### UC-5: Duplicate request

Повторный intent с тем же idempotency key не создаёт новый бюджетный расход и возвращает исходное решение.

### UC-6: Emergency freeze

Администратор замораживает agent, policy или workspace. Новые платежи получают `deny/frozen`; уже подписанные транзакции не могут быть отменены, что должно быть явно показано пользователю.

### UC-7: Audit investigation

Аудитор фильтрует решения по agent, merchant, policy version, outcome, network, amount и времени, затем экспортирует JSON/CSV evidence.

## 6. Функциональные требования

### FR-1. Identity и scope

- Каждый agent имеет стабильный `agent_id` и credential.
- Credential ограничен workspace и environment.
- Requests без валидной identity отклоняются.
- Agent не может сам изменить собственную policy.

### FR-2. Policy rules

Policy должна поддерживать:

- максимальную сумму одного платежа;
- дневной, месячный и session budget;
- allowlist merchant origin, endpoint и payTo;
- разрешённые network и asset;
- разрешённые agent/tool scopes;
- approval threshold;
- velocity limit и минимальный интервал между платежами;
- freeze/revoke;
- policy effective time и expiry;
- default-deny для неизвестных значений.

### FR-3. Deterministic evaluator

- одинаковый input и одинаковая активная policy дают одинаковое решение;
- результат содержит `allow`, `deny` или `approval_required`;
- каждая причина имеет стабильный code и человекочитаемое explanation;
- более строгий лимит побеждает менее строгий;
- отсутствие данных не трактуется как разрешение.

### FR-4. Payment intent

Intent содержит:

- `intent_id`;
- `idempotency_key`;
- workspace, environment и agent IDs;
- endpoint и merchant identity;
- network, asset, amount и decimals;
- quote expiry;
- request hash без секретных payloads;
- requested capabilities;
- creation timestamp.

### FR-5. Decision token

Для `allow` система выдаёт короткоживущий signed decision token, содержащий:

- decision ID;
- intent hash;
- policy version;
- expiry;
- allowed amount, merchant и network;
- nonce;
- signature key ID.

Token нельзя использовать для другого intent или после expiry.

### FR-6. Settlement evidence

SDK принимает результат:

- decision ID;
- transaction signature/hash;
- facilitator result;
- settlement status;
- observed amount и merchant;
- timestamps;
- error code при failure.

Неудачный или отсутствующий settlement не помечается как успешный платёж.

### FR-7. Idempotency и replay protection

- Повтор intent возвращает исходное decision.
- Повтор settlement безопасен.
- Nonce и TTL препятствуют reuse decision token.
- Mismatch между approved intent и settlement создаёт audit finding.

### FR-8. Dashboard

MVP dashboard включает:

- список workspaces, environments и agents;
- policy editor с preview decision;
- список merchants и allowlist;
- usage counters и budget remaining;
- pending approvals;
- freeze/unfreeze;
- audit explorer;
- export evidence;
- onboarding и sandbox demo.

### FR-9. API

Минимальные endpoints:

```text
POST /v1/intents/evaluate
POST /v1/intents/{intent_id}/settlement
GET  /v1/decisions/{decision_id}
GET  /v1/audit/events
POST /v1/agents
POST /v1/policies
PUT  /v1/policies/{policy_id}
POST /v1/policies/{policy_id}/freeze
POST /v1/approvals/{approval_id}/approve
POST /v1/approvals/{approval_id}/deny
```

Все mutating endpoints требуют idempotency key, где операция допускает повтор.

## 7. Пример API decision

### Request

```json
{
  "idempotency_key": "req_01J...",
  "agent_id": "agent_researcher",
  "environment_id": "env_sandbox",
  "merchant": "api.example.com",
  "pay_to": "merchant-public-key",
  "network": "solana-devnet",
  "asset": "USDC",
  "amount_atomic": "250000",
  "quote_expires_at": "2026-11-04T12:00:00Z",
  "request_hash": "sha256:..."
}
```

### Allow response

```json
{
  "decision": "allow",
  "decision_id": "dec_01J...",
  "policy_version": 12,
  "expires_at": "2026-11-04T12:00:30Z",
  "reason_code": "within_budget_and_allowlist",
  "decision_token": "signed-token"
}
```

### Deny response

```json
{
  "decision": "deny",
  "decision_id": "dec_01J...",
  "reason_code": "daily_budget_exceeded",
  "message": "The requested amount exceeds the remaining daily budget.",
  "retryable": false
}
```

## 8. Data model

Основные сущности:

- `workspaces` — владелец и tenant boundary;
- `environments` — sandbox, devnet, production;
- `agents` — delegated identities;
- `credentials` — hashed/revocable access credentials;
- `policies` — immutable versions;
- `policy_rules` — normalized deterministic rules;
- `merchants` — verified allowlist entries;
- `payment_intents` — requested operations;
- `decisions` — immutable allow/deny/approval result;
- `approvals` — human approval lifecycle;
- `settlements` — payment outcome evidence;
- `budget_counters` — atomic usage windows;
- `audit_events` — append-only security trail;
- `signing_keys` — public keys for decision signatures;
- `freeze_controls` — workspace/agent/policy emergency state.

Все таблицы tenant-scoped. Audit events не обновляются и не удаляются обычным API.

## 9. Non-functional requirements

### Security

- default deny;
- private keys никогда не принимаются и не логируются;
- credentials хранятся только в hashed form;
- encryption in transit and at rest;
- RBAC для owner/admin/operator/approver/auditor;
- immutable policy versions;
- signed decision tokens;
- audit tamper evidence;
- rate limits и replay protection;
- secrets redaction в логах и telemetry.

### Reliability

- evaluator должен быть deterministic и horizontally scalable;
- budget increment и decision creation выполняются атомарно;
- при недоступности policy service платеж не разрешается автоматически;
- settlement retry безопасен;
- backup/restore сохраняет audit ordering и policy versions.

### Performance

Цели MVP:

- p95 policy evaluation < 100 ms без внешнего RPC;
- p99 < 300 ms;
- dashboard read p95 < 500 ms;
- audit ingestion не блокирует уже завершённый merchant response;
- поддержка 100 decisions/sec на одном production instance.

### Privacy

- не сохранять полные paid API responses;
- request payload хранить только как hash или redacted metadata;
- configurable retention;
- export должен исключать credentials и секретные headers.

## 10. Архитектура MVP

Компоненты:

1. **SDK middleware** — перехватывает x402 quote и вызывает policy API.
2. **API gateway** — authentication, rate limits, request validation.
3. **Policy evaluator** — deterministic rule evaluation.
4. **Budget service** — atomic counters и time windows.
5. **Approval service** — pending decisions и TTL.
6. **Audit writer** — append-only events и hash chaining.
7. **PostgreSQL** — control plane и durable evidence.
8. **Dashboard** — policies, approvals, usage и investigation.
9. **Key service** — public signing keys и rotation metadata; не wallet custody.

## 11. MVP workflow

```text
x402 client
    ↓
SDK middleware
    ↓
POST /intents/evaluate
    ↓
Auth → validate → load policy → merchant check
    ↓
Atomic budget reservation
    ↓
allow / deny / approval_required
    ↓
Customer-owned signer
    ↓
Facilitator / merchant
    ↓
Settlement evidence → audit + counter finalization
```

Если settlement не подтверждён, reserved budget должен перейти в состояние `unknown` или быть освобождён по явно заданному правилу, но не считаться успешной оплатой автоматически.

## 12. Ошибки и reason codes

Минимальный каталог:

- `invalid_credential`;
- `agent_frozen`;
- `workspace_frozen`;
- `merchant_not_allowlisted`;
- `network_not_allowed`;
- `asset_not_allowed`;
- `per_payment_limit_exceeded`;
- `daily_budget_exceeded`;
- `monthly_budget_exceeded`;
- `velocity_limit_exceeded`;
- `approval_required`;
- `decision_expired`;
- `duplicate_intent`;
- `settlement_mismatch`;
- `facilitator_rejected`;
- `settlement_unknown`;
- `policy_version_conflict`.

## 13. Безопасность платежного цикла

1. Quote не является разрешением на оплату.
2. Merchant identity нормализуется и проверяется до policy evaluation.
3. `pay_to` проверяется отдельно от URL.
4. Approved amount сравнивается с фактически подписанным amount.
5. Decision token привязан к intent hash.
6. Один idempotency key допускает только один intent outcome.
7. Approval нельзя расширить после выдачи.
8. Freeze действует fail-closed.
9. Все изменения policy получают новую immutable version.
10. Audit содержит actor, reason, policy version и correlation IDs.

## 14. Аналитика продукта

Отслеживать:

- evaluated intents;
- allow/deny/approval rates;
- deny reasons;
- reserved vs settled amounts;
- duplicate attempts;
- settlement mismatch;
- time to approval;
- policy changes;
- freeze events;
- active agents и merchants.

Аналитические события не должны содержать bearer tokens, private keys, seed phrases или полные платёжные payloads.

## 15. Acceptance criteria MVP

MVP считается готовым, если:

1. Агент может выполнить разрешённый sandbox x402 request через SDK.
2. Платёж выше per-call cap блокируется до подписи.
3. Платёж неизвестному merchant блокируется до подписи.
4. Daily budget не обходится параллельными запросами.
5. Повторный intent не расходует бюджет повторно.
6. Approval имеет TTL и audit trail.
7. Kill switch блокирует новые решения менее чем за 1 секунду после propagation.
8. Settlement mismatch создаёт finding.
9. Dashboard показывает decision, reason, policy version и settlement status.
10. Ни один автоматический тест не находит секретов в логах, response или database payloads.
11. Полный sandbox demo запускается без реальных денег.
12. Интеграция с существующим x402 client требует не более 20 строк adapter-кода для базового сценария.

## 16. Roadmap

### Phase 1 — Discovery и policy core

- interview research;
- policy schema;
- deterministic evaluator;
- local SDK adapter;
- audit event model.

### Phase 2 — Sandbox MVP

- API gateway;
- PostgreSQL persistence;
- budget atomicity;
- allow/deny decisions;
- fake facilitator and test merchant;
- dashboard basics.

### Phase 3 — Real x402 integration

- devnet facilitator integration;
- signed decision token;
- settlement evidence;
- retry and idempotency;
- multi-agent test scenarios.

### Phase 4 — Pilot hardening

- approvals;
- kill switch;
- RBAC;
- audit export;
- security review;
- load and failure tests.

### Phase 5 — Commercial packaging

- hosted deployment;
- self-hosted distribution;
- SDKs for TypeScript and Rust;
- documentation and onboarding;
- support workflows;
- optional plans and usage limits.

## 17. Risks

- x402 ecosystem changes quickly;
- wallet providers may add equivalent controls;
- customer teams may prefer smart-wallet controls over an off-chain policy layer;
- custody expectations can create regulatory exposure;
- incorrect merchant normalization can create false approvals;
- availability failure must fail closed without making the agent unusable;
- audit data may contain more sensitive metadata than expected.

## 18. Open questions

1. Нужен ли отдельный hosted product или достаточно SDK + self-hosted API?
2. Какой минимальный набор chains кроме Solana должен быть в MVP?
3. Должны ли approvals быть synchronous или только asynchronous?
4. Как customer будет подтверждать merchant identity?
5. Какой settlement status считается достаточным для budget finalization?
6. Нужен ли on-chain policy anchor или достаточно signed off-chain audit?
7. Какой deployment и support model готовы оплачивать первые клиенты?

