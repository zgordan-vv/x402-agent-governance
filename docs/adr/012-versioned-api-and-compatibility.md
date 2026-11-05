# ADR-012: Versioned API and Compatibility Policy

- **Status:** Accepted
- **Date:** 2026-11-05

## Context

Guardrail has several independently deployed surfaces: the TypeScript SDK, future Rust SDK, dashboard, API service, facilitator adapters, and decision-token verifiers. A breaking change in one surface can cause an agent to stop paying, misinterpret a denial, or incorrectly treat settlement evidence as success.

The product also needs to evolve its policy schema and database without rewriting historical decisions.

## Decision

The public HTTP API is versioned by path (`/v1`). Within a major API version:

- existing fields retain their meaning;
- new response fields are additive and optional;
- new enum values are documented as possible unknown values for clients;
- required request fields are not added without a compatibility plan;
- error codes are stable identifiers, while messages may improve;
- deprecation is announced before removal.

Canonical intent and decision-token formats have explicit protocol versions. A token verifier must reject unsupported versions rather than silently interpreting fields under the wrong rules.

## Compatibility rules

### API

Breaking changes require `/v2` or an explicitly negotiated media type. The service may support multiple versions during a migration window.

### SDK

SDKs use semantic versioning. A patch release fixes behavior without changing the public contract; a minor release adds backward-compatible features; a major release may remove or alter APIs.

### Policies

Published policy versions are immutable. A policy schema migration creates a new schema version and a conversion tool; historical policies remain readable for replay.

### Database

Migrations follow expand/contract:

1. add nullable/new structures;
2. deploy code that writes both representations when needed;
3. backfill and verify;
4. switch reads;
5. remove old structures only after the compatibility window.

### Decision tokens

Tokens include a protocol version and `kid`. Verifiers support the active version and a bounded set of previous versions until their maximum TTL has elapsed. No token format change may invalidate an already issued token before its expiry.

## Contract testing

CI must run:

- OpenAPI schema compatibility checks;
- SDK-to-API integration tests;
- canonical intent vectors shared by Rust and TypeScript;
- decision-token verification vectors;
- migration tests from a clean database and from the previous release;
- replay tests for historical policy versions.

## Rejected alternatives

### Unversioned API

Rejected because SDKs and customer agents cannot safely coordinate breaking changes.

### Breaking changes in minor releases

Rejected because payment control software requires predictable upgrades.

### Rewriting historical records to the newest schema

Rejected because it would compromise audit reproducibility and incident investigation.

### Supporting every historical version forever

Rejected because security fixes and operational complexity would become unbounded. Deprecation windows must be explicit.

## Consequences

Positive:

- safer SDK and dashboard upgrades;
- reproducible historical decisions;
- coordinated Rust/TypeScript implementations;
- controlled database migrations;
- clear customer communication.

Negative:

- duplicate compatibility code during migration windows;
- more contract and vector tests;
- old token keys and schemas need bounded retention;
- deprecation documentation becomes a release responsibility.

## Required tests

- additive response changes do not break old SDK fixtures;
- unsupported API/token versions fail explicitly;
- old tokens verify until expiry;
- migrations work from the previous release;
- policy replay uses the original schema/version;
- error codes remain stable across a compatible release.

## Revisit when

Revisit if the protocol becomes governed by an external standard that provides an incompatible version-negotiation mechanism, or if customer deployment constraints require a different release strategy.
