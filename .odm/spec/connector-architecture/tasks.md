<!--
domain: connector-architecture
updated: 2026-03-22
spec-brief: ./brief.md
-->

# Implementation Plan — Connector Architecture

## Task Overview

This plan implements the connector architecture for Life Engine. Work begins with the shared connector infrastructure (OAuth flow, credential bridge, rate limiting) and then builds the first concrete connector (IMAP email). Each task targets 1-3 files and produces a testable outcome.

**Progress:** 0 / 14 tasks complete

## Steering Document Compliance

- Connectors are regular plugins following Open/Closed Principle — no special connector category in Core
- OAuth tokens are encrypted at rest with Defence in Depth — refresh tokens in SQLCipher, access tokens memory-only
- All token operations are audit-logged per Explicit Over Implicit
- Protocol-first approach follows Finish Before Widening — one IMAP connector covers all email providers

## Atomic Task Requirements

- **File Scope:** 1-3 related files maximum
- **Time Boxing:** 15-30 minutes per task
- **Single Purpose:** one testable outcome per task
- **Specific Files:** exact file paths specified
- **Agent-Friendly:** clear input/output, minimal context switching

---

## 1.1 — Connector Types and State Machine

> spec: ./brief.md

- [ ] Define connector state enum and lifecycle transitions
  <!-- file: packages/types/src/connector.rs -->
  <!-- purpose: Define ConnectorState enum (Registered, Syncing, Active, AuthExpired, Disconnected) with valid transitions -->
  <!-- requirements: 5.1, 5.2, 5.3, 5.4, 5.5 -->
  <!-- leverage: existing packages/types/src/lib.rs module structure -->

- [ ] Define connector configuration types
  <!-- file: packages/types/src/connector.rs -->
  <!-- purpose: Add ConnectorConfig struct with sync_interval, rate_limit, and allowed_domains fields -->
  <!-- requirements: 4.1, 6.1 -->
  <!-- leverage: none -->

## 1.2 — OAuth PKCE Flow

> spec: ./brief.md

- [ ] Implement OAuth PKCE challenge generation and callback handling
  <!-- file: apps/core/src/auth/oidc.rs -->
  <!-- purpose: Add PKCE code_verifier/code_challenge generation and authorization_code exchange -->
  <!-- requirements: 1.1, 1.2 -->
  <!-- leverage: existing apps/core/src/auth/oidc.rs OIDC implementation -->

- [ ] Add OAuth callback route for connector authentication
  <!-- file: apps/core/src/routes/connectors.rs -->
  <!-- purpose: Handle OAuth redirect callback, exchange code for tokens, store via credential bridge -->
  <!-- requirements: 1.1, 1.2, 1.3 -->
  <!-- leverage: existing apps/core/src/routes/ route structure -->

## 1.3 — Credential Bridge Enhancement

> spec: ./brief.md

- [ ] Add token refresh and automatic rotation to credential bridge
  <!-- file: apps/core/src/credential_bridge.rs -->
  <!-- purpose: Implement automatic token refresh when access token is within 5 minutes of expiry -->
  <!-- requirements: 1.4, 1.5 -->
  <!-- leverage: existing apps/core/src/credential_bridge.rs -->

- [ ] Add token revocation to credential bridge
  <!-- file: apps/core/src/credential_bridge.rs, apps/core/src/credential_store.rs -->
  <!-- purpose: Implement local token deletion and provider revocation endpoint call on disconnect -->
  <!-- requirements: 5.4 -->
  <!-- leverage: existing credential_bridge.rs and credential_store.rs -->

## 2.1 — Normalisation Pipeline

> spec: ./brief.md

- [ ] Create normalisation trait and helpers for canonical collection writes
  <!-- file: apps/core/src/connector.rs -->
  <!-- purpose: Define Normaliser trait with normalize() method that maps raw responses to canonical schemas -->
  <!-- requirements: 2.1, 2.2, 2.3 -->
  <!-- leverage: existing apps/core/src/connector.rs -->

- [ ] Add validation and error handling for normalisation failures
  <!-- file: apps/core/src/connector.rs -->
  <!-- purpose: Validate normalised records against canonical schema, log and skip invalid records -->
  <!-- requirements: 2.4 -->
  <!-- leverage: existing apps/core/src/schema_registry.rs -->

## 2.2 — Raw Data Storage

> spec: ./brief.md

- [ ] Implement raw data write path with private collection namespacing
  <!-- file: apps/core/src/connector.rs, apps/core/src/storage.rs -->
  <!-- purpose: Write raw API responses to plugin-namespaced raw_data collection with required metadata fields -->
  <!-- requirements: 3.1, 3.2, 3.4 -->
  <!-- leverage: existing storage.rs plugin_data writes -->

- [ ] Add raw data reprocessing support
  <!-- file: apps/core/src/connector.rs -->
  <!-- purpose: Add reprocess_raw_data() method that reads raw records and re-normalises them into canonical collections -->
  <!-- requirements: 3.3 -->
  <!-- leverage: normalisation trait from task 2.1 -->

## 3.1 — Sync Engine

> spec: ./brief.md

- [ ] Implement periodic sync scheduler integration
  <!-- file: apps/core/src/connector.rs -->
  <!-- purpose: Register periodic sync jobs with the background scheduler at the configured interval -->
  <!-- requirements: 4.1, 4.2 -->
  <!-- leverage: existing background scheduler infrastructure -->

- [ ] Implement rate limiting and exponential backoff for outbound requests
  <!-- file: apps/core/src/rate_limit.rs -->
  <!-- purpose: Add per-service rate limiter with Retry-After header support and exponential backoff -->
  <!-- requirements: 4.4, 4.5, 6.1, 6.2, 6.3 -->
  <!-- leverage: existing apps/core/src/rate_limit.rs -->

## 3.2 — Audit Logging for Token Operations

> spec: ./brief.md

- [ ] Add audit log entries for all credential operations
  <!-- file: apps/core/src/credential_bridge.rs -->
  <!-- purpose: Log credential.read, credential.rotate, and credential.revoke events to audit_log table -->
  <!-- requirements: 7.1, 7.2, 7.3 -->
  <!-- leverage: existing audit_log table and logging infrastructure -->

## 4.1 — IMAP Email Connector

> spec: ./brief.md

- [ ] Scaffold IMAP connector plugin with manifest and IMAP connection
  <!-- file: plugins/life/imap-connector/plugin.json, plugins/life/imap-connector/src/lib.rs -->
  <!-- purpose: Create IMAP connector plugin that connects to mail servers and fetches inbox message list -->
  <!-- requirements: 8.1, 8.2, 8.3, 8.4 -->
  <!-- leverage: existing plugins/ directory structure -->
