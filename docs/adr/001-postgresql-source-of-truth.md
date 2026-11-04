# ADR-001: PostgreSQL как source of truth

- **Статус:** Accepted
- **Дата:** 2026-11-04
- **Область:** persistence, budgets, decisions, audit
- **Связанные документы:** `docs/prd.md`, `docs/system-design.md`

## Контекст

Guardrail должен принимать решения о платежах до подписи транзакции и одновременно вести бюджет агента, idempotency state, approval lifecycle и audit evidence.

Критические операции должны быть согласованными:

1. один idempotency key не должен создать две оплаты;
2. параллельные запросы не должны превысить daily/per-payment budget;
3. decision и budget reservation должны либо сохраниться вместе, либо не сохраниться вообще;
4. settlement evidence не должен превратить неизвестный результат в успешный;
5. audit event должен ссылаться на ту же policy version и decision, которые реально использовались.

Система также должна поддерживать сложные tenant-scoped queries: audit filters, policy history, approvals, agent usage и reconciliation of unknown settlements.

## Решение

PostgreSQL 16+ выбирается как authoritative source of truth для:

- workspaces и environments;
- agents и credential metadata;
- immutable policy versions;
- merchants и allowlists;
- payment intents и idempotency keys;
- decisions;
- budget counters и reservations;
- approvals;
- settlement evidence;
- append-only audit events.

Atomic evaluate flow выполняется внутри PostgreSQL transaction:

```text
BEGIN
  load idempotent intent if it exists
  lock relevant budget counter rows FOR UPDATE
  load active policy and freeze state
  evaluate normalized intent
  insert intent and immutable decision
  reserve budget when outcome is allow/approval_required
  append audit event
COMMIT
```

Если любой шаг не проходит, transaction откатывается. Нельзя иметь decision без соответствующей reservation или reservation без decision.

PostgreSQL является source of truth, но не обязан обслуживать каждый read напрямую:

- Redis может использоваться как временный cache или distributed rate limiter;
- read replicas могут обслуживать тяжёлые audit queries;
- object storage может хранить архивные exports и backup files.

Ни один из этих компонентов не может единолично разрешать платёж или заменять authoritative budget state.

## Почему PostgreSQL

### 1. Транзакционная согласованность

Budget reservation, decision и audit event требуют ACID transaction. PostgreSQL предоставляет row locks, isolation levels, constraints и rollback без отдельного distributed transaction coordinator.

### 2. Уникальность и idempotency на уровне базы

Уникальные ограничения вроде:

```sql
UNIQUE (workspace_id, agent_id, idempotency_key)
UNIQUE (aggregate_id, sequence)
```

защищают от ошибок приложения и race conditions.

### 3. Богатые запросы для governance

Продукту нужны фильтры по agent, merchant, policy version, reason code, status и времени. Реляционная модель подходит для этих запросов лучше, чем key-value storage или только event log.

### 4. Безопасные constraints

Database может enforce-ить foreign keys, non-negative amounts, допустимые statuses и tenant relationships. Это второй защитный слой после Rust domain validation.

### 5. Операционная зрелость

PostgreSQL поддерживает managed hosting, backups, point-in-time recovery, replicas, TLS, role-based access и инструменты миграций.

### 6. Простота MVP

Одна PostgreSQL-инстанция проще для локального Docker Compose, pilot deployment и disaster-recovery rehearsal, чем комбинация нескольких специализированных хранилищ.

## Рассмотренные альтернативы

### Redis как source of truth

**Отклонено.** Redis полезен для ephemeral cache и rate limiting, но неудобен для долговременного audit, сложных tenant queries, immutable history и backup semantics. Redis-only модель также повышает риск потерять budget state при ошибке persistence или неправильном failover.

### MongoDB/document database

**Отклонено для core ledger.** Гибкая schema полезна для raw metadata, но основная модель имеет строгие связи, uniqueness и atomic counters. Реляционные constraints здесь ценнее schema flexibility.

### Kafka/event log как source of truth

**Отклонено для MVP.** Event streaming хорошо подходит для high-throughput integration, но сам по себе не даёт удобного transaction boundary между budget reservation, decision и audit. Kafka может появиться позже для outbound analytics, но не должна первой принимать payment decision.

### Blockchain как source of truth

**Отклонено.** x402 settlement происходит on-chain, но policy decision, daily budget, private workspace state и approval workflow не должны зависеть от latency, fees или доступности конкретной сети. Blockchain используется для payment evidence, а не для всей control-plane state.

### Только append-only event store

**Отклонено для MVP.** Event sourcing возможен, но добавляет сложность projection rebuild, idempotent command handling и operational recovery. Audit chain поверх PostgreSQL даёт достаточную tamper evidence без преждевременной сложности.

## Consequences

### Положительные

- atomic budget reservation;
- понятные migrations и rollback strategy;
- database-enforced tenant isolation;
- удобные audit и reporting queries;
- простая local development story;
- зрелые backup/restore инструменты.

### Отрицательные

- PostgreSQL становится критической зависимостью для evaluate path;
- высокие write rates потребуют partitioning, replicas и connection-pool tuning;
- audit retention может быстро увеличить storage;
- distributed multi-region writes не являются частью MVP;
- database outage приводит к fail-closed отказу новых платежей.

## Operational safeguards

1. `DATABASE_URL` и database credentials находятся только в secret manager.
2. API использует отдельную least-privilege role.
3. Migration role отделена от runtime role.
4. Включены TLS, backups и point-in-time recovery.
5. Budget transaction покрыта concurrency tests.
6. Для больших audit tables предусмотрены monthly partitions.
7. Read replicas не используются для authoritative decision или budget checks.
8. Redis cache никогда не является основанием для `allow`.
9. Restore test выполняется до production release.

## Когда пересмотреть решение

ADR нужно пересмотреть, если:

- требуется более 10,000 decision evaluations/sec на tenant;
- multi-region active-active writes становятся обязательными;
- audit retention превышает возможности PostgreSQL partitioning;
- customer требует deployment без relational database;
- реальные benchmarks показывают, что PostgreSQL transaction latency нарушает SLO.

Даже тогда PostgreSQL может остаться authoritative ledger, а высокопроизводительный evaluator — использовать отдельный read model с безопасной revalidation перед выдачей decision token.

