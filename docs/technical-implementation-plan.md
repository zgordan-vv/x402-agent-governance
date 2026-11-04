# Technical Implementation Plan: x402 Agent Spend Governance

Этот документ превращает PRD и system design в исполнимый план разработки. Каждый шаг имеет конкретный результат и критерий завершения.

## 1. Целевая архитектура и tech stack

### Backend

- **Rust 1.86+** — основной язык backend и evaluator.
- **Axum** — HTTP routing, extractors, middleware.
- **Tokio** — async runtime.
- **Serde/serde_json** — API и policy schemas.
- **SQLx** — PostgreSQL access, migrations и compile-time checked queries там, где это возможно.
- **PostgreSQL 16+** — source of truth для policies, reservations, decisions и audit.
- **Tower/Tower-HTTP** — tracing, CORS, request IDs, timeouts и rate-limit integration.
- **tracing + tracing-subscriber** — structured logs.
- **uuid** с UUIDv7 — sortable entity IDs.
- **sha2/blake3** — canonical hashes и audit chain.
- **ed25519-dalek** — signing decision tokens.
- **argon2** — credential hashing.
- **thiserror/anyhow** — typed domain errors и application context.

### SDK

- **TypeScript** — основной public SDK для x402 clients.
- **Node.js 20+** — supported runtime.
- **Web Crypto/Node crypto** — hashing и local idempotency helpers.
- **Fetch API** — transport без обязательного framework.
- **Rust crate** — второй SDK после стабилизации protocol types.

### Dashboard

- **React + TypeScript**.
- **Vite** — build/dev server.
- **React Router** — routes.
- **TanStack Query** — API cache, retries и invalidation.
- **Zod** — runtime validation API responses.
- **CSS modules или scoped CSS** — predictable component styles.

### Infrastructure

- **Docker Compose** — local development и reproducible integration tests.
- **Docker multi-stage builds** — production images.
- **Managed PostgreSQL** — pilot deployment.
- **Object storage** — audit exports и backups.
- **Reverse proxy with HTTPS** — public API/dashboard.
- **GitHub Actions** — formatting, lint, tests, migrations check, image build.

### Observability и security

- **OpenTelemetry-compatible traces/metrics**.
- **Prometheus metrics format**.
- **Sentry или equivalent error tracking**.
- **Secret manager/KMS** для decision signing key.
- **Trivy** для container scan.
- **cargo-audit/cargo-deny** для Rust dependencies.
- **npm/pnpm audit** для SDK/dashboard dependencies.

## 2. Repository structure

```text
x402-agent-governance/
├── Cargo.toml
├── Cargo.lock
├── package.json
├── pnpm-workspace.yaml
├── docker-compose.yml
├── Dockerfile
├── .env.example
├── crates/
│   ├── governance-core/
│   │   ├── src/
│   │   │   ├── lib.rs
│   │   │   ├── policy.rs
│   │   │   ├── evaluator.rs
│   │   │   ├── budget.rs
│   │   │   ├── decision.rs
│   │   │   ├── merchant.rs
│   │   │   ├── errors.rs
│   │   │   └── canonical.rs
│   │   └── tests/
│   ├── governance-api/
│   │   └── src/
│   │       ├── main.rs
│   │       ├── state.rs
│   │       ├── auth.rs
│   │       ├── routes/
│   │       └── http_error.rs
│   ├── governance-storage/
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── repositories.rs
│   │       └── transactions.rs
│   ├── governance-worker/
│   │   └── src/main.rs
│   └── governance-sdk-rust/
│       └── src/lib.rs
├── migrations/
├── packages/
│   └── x402-governance-sdk/
│       ├── src/
│       │   ├── client.ts
│       │   ├── types.ts
│       │   ├── middleware.ts
│       │   └── errors.ts
│       └── test/
├── apps/
│   └── dashboard/
│       └── src/
│           ├── api/
│           ├── routes/
│           ├── components/
│           └── main.tsx
├── fixtures/
├── scripts/
├── docs/
└── .github/workflows/
```

## 3. Development rules

1. Сначала domain types и invariants, затем HTTP и UI.
2. Не добавлять database shortcut, который обходит evaluator.
3. Все state-changing API — idempotent.
4. Никаких private keys, seed phrases или bearer tokens в fixtures.
5. Каждая новая reason code должна иметь unit test и UI explanation.
6. Каждое изменение schema должно иметь reversible migration strategy.
7. Не объявлять feature готовой без negative-path test.

## 4. Phase 0 — Workspace bootstrap

### Step 0.1 — Create repository metadata

Создать:

- `Cargo.toml` workspace;
- `package.json` и pnpm workspace;
- `.editorconfig`;
- `rustfmt.toml`;
- `clippy.toml`;
- `.gitignore`;
- `.env.example` без секретов;
- `README.md` с local quickstart.

Результат: repository компилируется и содержит минимальные Rust/TypeScript packages.

### Step 0.2 — Configure quality gates

Добавить scripts:

```text
cargo fmt --check
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
pnpm lint
pnpm typecheck
pnpm test
```

Результат: CI может отказать на formatting, type errors и failing tests.

### Step 0.3 — Create local Compose

Services:

- `postgres`;
- `api`;
- `worker`;
- `dashboard`.

Добавить healthchecks и named volume только для PostgreSQL. Секреты передаются через `.env`, который не коммитится.

## 5. Phase 1 — Domain model и canonical protocol

### Step 1.1 — Define identifiers and money types

Создать в `governance-core`:

- `WorkspaceId`, `EnvironmentId`, `AgentId`;
- `PolicyId`, `IntentId`, `DecisionId`;
- `AmountAtomic` как checked integer/string boundary;
- `Network`, `Asset`, `DecisionOutcome`.

Нельзя использовать floating point для денежных значений.

### Step 1.2 — Define normalized merchant

Реализовать `MerchantIdentity::normalize`:

- lowercase host;
- remove default ports;
- normalize scheme;
- preserve path prefix explicitly;
- normalize payTo separately;
- reject URL userinfo and malformed origins.

Тесты должны покрыть redirects, case differences, trailing slash, invalid URL и unicode host.

### Step 1.3 — Define intent canonicalization

Создать canonical representation без secret headers и response body.

```rust
pub struct PaymentIntentInput {
    pub agent_id: AgentId,
    pub merchant: MerchantIdentity,
    pub network: Network,
    pub asset: Asset,
    pub amount_atomic: AmountAtomic,
    pub quote_expires_at: DateTime<Utc>,
    pub request_hash: Hash,
}
```

`intent_hash` рассчитывается только из canonical fields в фиксированном порядке.

### Step 1.4 — Define reason codes

Создать enum `ReasonCode` с сериализацией в stable snake_case. Для каждого значения добавить:

- title;
- operator explanation;
- retryability;
- severity;
- documentation link.

## 6. Phase 2 — Policy schema и evaluator

### Step 2.1 — Policy schema

Определить:

```rust
pub struct PolicyRules {
    pub per_payment_cap: AmountAtomic,
    pub daily_cap: AmountAtomic,
    pub monthly_cap: Option<AmountAtomic>,
    pub session_cap: Option<AmountAtomic>,
    pub approval_threshold: Option<AmountAtomic>,
    pub velocity: Option<VelocityRule>,
    pub allowed_networks: Vec<Network>,
    pub allowed_assets: Vec<Asset>,
    pub merchants: Vec<MerchantRule>,
}
```

Semantic validation:

- `daily_cap >= per_payment_cap`;
- approval threshold not greater than daily cap;
- non-empty network/asset sets;
- no contradictory merchant rules;
- bounded numeric values.

### Step 2.2 — Evaluator context

```rust
pub struct EvaluationContext {
    pub now: DateTime<Utc>,
    pub agent_status: AgentStatus,
    pub workspace_frozen: bool,
    pub policy: ActivePolicy,
    pub intent: PaymentIntentInput,
    pub budget: BudgetSnapshot,
}
```

### Step 2.3 — Deterministic evaluation order

Реализовать функции:

```text
check_identity
check_freeze
check_policy_validity
check_network_asset
check_merchant
check_amount
check_budget
check_velocity
classify_approval
```

Внешний метод возвращает `DecisionResult`. Порядок не должен зависеть от HashMap iteration.

### Step 2.4 — Evaluator tests

Минимум:

- allow within all limits;
- deny per-payment cap;
- deny daily cap;
- deny unknown merchant;
- deny frozen agent;
- approval threshold;
- expired quote;
- network/asset mismatch;
- deterministic output for repeated evaluation.

## 7. Phase 3 — PostgreSQL schema и storage

### Step 3.1 — Initial migration

Создать schemas `control`, `policy`, `payments`, `audit` и tables:

- workspaces;
- environments;
- agents;
- credentials;
- policy_versions;
- merchants;
- payment_intents;
- decisions;
- budget_counters;
- budget_reservations;
- approvals;
- settlements;
- audit_events.

Каждая таблица получает `created_at`; mutable tables — `updated_at`.

### Step 3.2 — Constraints и indexes

Добавить:

- foreign keys;
- status check constraints;
- unique `(workspace_id, agent_id, idempotency_key)`;
- unique `(aggregate_id, sequence)`;
- amount non-negative checks;
- tenant composite indexes.

### Step 3.3 — Repository traits

Создать traits:

```rust
trait AgentRepository { ... }
trait PolicyRepository { ... }
trait IntentRepository { ... }
trait BudgetRepository { ... }
trait DecisionRepository { ... }
trait SettlementRepository { ... }
trait AuditRepository { ... }
```

Domain layer не должен зависеть от SQLx types.

### Step 3.4 — Atomic evaluate transaction

Реализовать `evaluate_and_reserve`:

1. получить idempotent existing intent;
2. открыть transaction;
3. lock budget counter row;
4. load policy and freeze state;
5. run evaluator;
6. insert intent and decision;
7. increment reservation;
8. insert audit event;
9. commit.

Negative tests должны доказать, что rollback не оставляет частичную reservation.

## 8. Phase 4 — Authentication и authorization

### Step 4.1 — Credential creation

API возвращает plaintext credential только при создании. В database сохраняется hash, prefix и metadata.

### Step 4.2 — Auth middleware

Middleware:

- extracts bearer token;
- hashes with constant-time-safe comparison;
- checks revoked/expired;
- loads workspace/agent scope;
- attaches `RequestPrincipal`.

### Step 4.3 — RBAC

Роли:

- owner;
- security_admin;
- operator;
- approver;
- auditor.

Добавить matrix tests для каждой route и роли.

### Step 4.4 — Rate limits

На первом этапе использовать per-credential token bucket в API process с database-safe fallback. Для нескольких replicas вынести limiter в Redis только после измерения необходимости.

## 9. Phase 5 — Axum API

### Step 5.1 — Application state

```rust
pub struct AppState {
    pub pool: PgPool,
    pub evaluator: Arc<PolicyEvaluator>,
    pub token_signer: Arc<DecisionTokenSigner>,
    pub clock: Arc<dyn Clock>,
}
```

Clock injection нужен для deterministic expiry tests.

### Step 5.2 — Routes

Реализовать routers:

```text
/health/live
/health/ready
/v1/intents/evaluate
/v1/intents/{intent_id}/settlement
/v1/policies
/v1/agents
/v1/approvals
/v1/audit
```

### Step 5.3 — Error envelope

Все domain errors маппятся в:

```json
{
  "error": {
    "code": "daily_budget_exceeded",
    "message": "The requested amount exceeds the remaining daily budget.",
    "retryable": false,
    "request_id": "req_..."
  }
}
```

### Step 5.4 — OpenAPI

Добавить route annotations и generated `/openapi.json`. Contract test проверяет основные schemas, statuses и reason codes.

## 10. Phase 6 — Signed decision tokens

### Step 6.1 — Key bootstrap

В development key создаётся из env secret. В production private signing key берётся из KMS/secret manager и не записывается в database.

### Step 6.2 — Token claims

```json
{
  "iss": "guardrail",
  "sub": "agent_researcher",
  "decision_id": "dec_...",
  "intent_hash": "sha256:...",
  "policy_version": 12,
  "amount_atomic": "250000",
  "pay_to": "...",
  "network": "solana-devnet",
  "exp": 1793793630,
  "nonce": "..."
}
```

### Step 6.3 — Verification

Создать verifier для facilitator adapter и тесты:

- signature valid;
- expired token;
- modified amount;
- modified payTo;
- wrong intent hash;
- unknown key ID;
- replayed nonce.

## 11. Phase 7 — Settlement и reconciliation

### Step 7.1 — Settlement endpoint

Принимает только evidence, связанную с существующим intent/decision. Проверяет credential scope и idempotency.

### Step 7.2 — Outcome state machine

```text
reserved → settled
reserved → failed
reserved → unknown
reserved → released
```

Запретить переходы назад и повторное финальное состояние.

### Step 7.3 — Mismatch detection

Сверять amount, payTo, network, asset и decision ID. Mismatch создаёт audit finding и alert.

### Step 7.4 — Reconciliation worker

Worker периодически находит `unknown` и expired reservations, запрашивает configured facilitator/observer adapter и повторяет settlement reconciliation idempotently.

## 12. Phase 8 — TypeScript SDK

### Step 8.1 — Public types

Экспортировать `X402Quote`, `PaymentIntent`, `Decision`, `SettlementEvidence`, `GuardrailClient`.

### Step 8.2 — `governedFetch`

Алгоритм:

1. вызвать resource;
2. распознать HTTP 402;
3. parse payment requirements;
4. вызвать `evaluate`;
5. остановиться на deny;
6. вызвать injected signer на allow;
7. повторить запрос с proof;
8. отправить settlement evidence.

### Step 8.3 — Retry policy

Retry разрешён только для network/5xx на evaluate и settlement. Нельзя повторять signer автоматически после ambiguous response без idempotency.

### Step 8.4 — SDK tests

- fake merchant;
- allow path;
- deny before signer invocation;
- approval path;
- repeated quote;
- settlement timeout;
- malformed 402;
- secret redaction.

## 13. Phase 9 — Dashboard

### Step 9.1 — App shell

Создать routes:

```text
/overview
/agents
/policies
/merchants
/approvals
/audit
/sandbox
```

### Step 9.2 — API client

Сделать typed client на основе generated OpenAPI или вручную typed wrappers с Zod validation. Никогда не показывать credential после initial creation.

### Step 9.3 — Policy editor

UI должен показывать diff draft vs active, semantic validation errors и decision preview на example intent.

### Step 9.4 — Approval UI

Показать amount, merchant, agent, reason, policy version и expiry. Approve/Deny требуют confirmation и optional comment.

### Step 9.5 — Audit UI

Таблица с filters, pagination, detail drawer и export status. Скрывать sensitive metadata по role.

## 14. Phase 10 — Worker и operational jobs

### Step 10.1 — Job table

Создать durable jobs с `pending`, `leased`, `complete`, `retryable`, `dead_letter`.

### Step 10.2 — Jobs

- expire approvals;
- finalize expired reservations;
- reconcile unknown settlements;
- generate exports;
- apply retention;
- publish metrics rollups.

### Step 10.3 — Retry semantics

Exponential backoff, max attempts, lease expiry и structured failure reason. Job handlers должны быть idempotent.

## 15. Phase 11 — Sandbox и test merchant

Создать локальный fake merchant, который:

- возвращает x402 quote;
- принимает signed test proof;
- может симулировать success, facilitator reject и timeout;
- не двигает реальные деньги.

Добавить `docker compose --profile sandbox up` и one-command walkthrough.

## 16. Phase 12 — Security hardening

### Step 12.1 — Threat model

Разобрать:

- stolen credential;
- compromised agent;
- prompt injection;
- merchant spoofing;
- replay;
- concurrent double spend;
- database tampering;
- KMS/key rotation failure;
- malicious oversized quote.

### Step 12.2 — Security tests

- cross-tenant ID substitution;
- role escalation;
- replay old decision token;
- mutate signed amount;
- race budget reservations;
- bypass freeze;
- secrets in logs;
- request smuggling and oversized payloads.

### Step 12.3 — Dependency and image scans

CI запускает `cargo audit`, `cargo deny`, `pnpm audit --prod` и Trivy. Critical findings блокируют release.

## 17. Phase 13 — Performance и reliability

### Step 13.1 — Load generator

Сгенерировать intents с controlled distribution по agents, merchants и budgets. Измерять p50/p95/p99.

### Step 13.2 — Concurrency tests

100 параллельных intents на один budget должны никогда не превысить cap.

### Step 13.3 — Failure injection

Инжектировать:

- PostgreSQL restart;
- deadlock/serialization conflict;
- token signing unavailable;
- audit insert failure;
- worker crash;
- facilitator timeout.

Проверять fail-closed и отсутствие duplicate reservation.

### Step 13.4 — Backup/restore

Проверить restore в чистую PostgreSQL и сохранить policy versions, decisions, reservations и audit chain.

## 18. Phase 14 — Real devnet integration

Только после sandbox acceptance:

1. настроить disposable devnet wallet клиента;
2. использовать tiny USDC/SOL amounts;
3. зарегистрировать facilitator adapter;
4. прогнать allow, deny и mismatch scenarios;
5. проверить explorer signature;
6. удалить credentials после теста или revoke их.

Production wallet и реальные средства не нужны для MVP demonstration.

## 19. Phase 15 — Deployment

### Step 15.1 — Production images

Multi-stage Dockerfiles с non-root user, pinned base images и healthcheck.

### Step 15.2 — Environment contract

```text
DATABASE_URL
APP_ENV
DECISION_SIGNING_KEY_REF
PUBLIC_BASE_URL
ALLOWED_ORIGINS
OTEL_EXPORTER_OTLP_ENDPOINT
AUDIT_RETENTION_DAYS
```

Не хранить production values в git.

### Step 15.3 — Managed PostgreSQL

Включить TLS, backups, PITR, restricted network, least-privilege role и migration job.

### Step 15.4 — Release process

1. CI build/test/scan;
2. migration dry run;
3. deploy immutable image;
4. readiness check;
5. smoke test;
6. rollback на предыдущий image при failure.

## 20. Phase 16 — Documentation и onboarding

Создать:

- quickstart на sandbox;
- SDK integration guide;
- policy cookbook;
- threat model;
- API reference;
- incident response runbook;
- backup/restore runbook;
- customer security questionnaire;
- architecture decision records.

Каждый public example должен быть runnable и не содержать настоящих credentials.

## 21. Phase 17 — Acceptance и release checklist

### Functional acceptance

- allow path работает;
- все deny reason codes отображаются;
- approval lifecycle завершён;
- freeze работает;
- duplicate не расходует бюджет;
- settlement mismatch обнаруживается;
- audit export воспроизводим.

### Technical acceptance

- `cargo test --workspace` проходит;
- SDK unit/integration tests проходят;
- dashboard build проходит;
- migrations применяются с нуля;
- restore test проходит;
- load target достигается;
- security scans без critical findings;
- secrets отсутствуют в git и artifacts.

### Product acceptance

- новый пользователь запускает sandbox менее чем за 10 минут;
- базовая SDK интеграция требует не более 20 строк adapter-кода;
- reason codes понятны без чтения исходников;
- demo показывает allow, deny, approval и audit;
- минимум три pilot workflows подключены.

## 22. Recommended implementation order

Не менять порядок без причины:

1. repository bootstrap;
2. domain types и canonicalization;
3. policy evaluator;
4. migrations и repositories;
5. atomic budget transaction;
6. authentication/RBAC;
7. evaluate API;
8. signed decision tokens;
9. settlement API;
10. TypeScript SDK;
11. fake merchant sandbox;
12. dashboard;
13. worker/reconciliation;
14. security and load tests;
15. devnet integration;
16. deployment and documentation.

Такой порядок позволяет получить работающий безопасный sandbox до подключения реальных x402 платежей.

