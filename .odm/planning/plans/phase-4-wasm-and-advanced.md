---
title: "Phase 4 — WASM and Advanced"
tags:
  - life-engine
  - planning
  - phase-4
  - wasm
  - federation
  - mobile
created: 2026-03-21
---

# Phase 4 — WASM and Advanced

## Goal

Untrusted community plugins run safely in WASM. Multi-user support. Federated sync. Mobile releases. The ecosystem is self-sustaining.

## Entry Criteria

- Phase 3 complete
- Plugin store operational
- Visual builder functional
- 5+ first-party plugins stable
- Ecosystem foundations proven

## Table of Contents

- [[#4.1 — WASM Plugin Runtime]]
- [[#4.2 — Plugin Signing and Verification]]
- [[#4.3 — Multi-User / Household Support]]
- [[#4.4 — GraphQL API Plugin]]
- [[#4.5 — Federated Sync (Hub-to-Hub)]]
- [[#4.6 — Encrypted Remote Backup]]
- [[#4.7 — Identity and Credential System]]
- [[#4.8 — Mobile Releases (iOS + Android)]]
- [[#Exit Criteria]]
- [[#Phase-Specific Risks]]

---

## Work Packages

### 4.1 — WASM Plugin Runtime

**Scope**

Integrate Extism as the WASM runtime for sandboxed plugin execution:

- **Host functions** — implement the following host functions available to WASM plugins: `store_read`, `store_write`, `store_query`, `store_delete`, `config_get`, `event_subscribe`, `event_emit`, `log_info`, `log_warn`, `log_error`, `http_request` — all scoped to the plugin's declared capabilities
- **Capability enforcement** — undeclared capabilities result in the host function being unavailable to the plugin (not just returning an error, but not linked at all)
- **Resource limits** — 64MB memory per plugin, 30-second execution timeout, rate limits on host function calls
- **Migration** — migrate first-party Core plugins to WASM to validate the runtime
- **SDK update** — update `plugin-sdk-rs` with WASM compilation targets and host function bindings

**Deliverables**

- Core plugins run in WASM sandboxes with enforced capability isolation
- Untrusted code cannot access anything beyond its declared capabilities

**Dependencies**

- Phase 3

---

### 4.2 — Plugin Signing and Verification

**Scope**

Secure the plugin supply chain:

- **Ed25519 signing** — sign `.wasm` bundles with Ed25519 keys, signature stored alongside the bundle
- **Verification before loading** — Core verifies the signature before loading any WASM plugin
- **Unsigned plugin handling** — unsigned plugins require explicit user opt-in via configuration flag
- **Revocation list** — maintain a revocation list of compromised plugin signatures
- **Manifest hash** — include the manifest hash in the signature to prevent post-signing tampering of capability declarations
- **Verification tiers** — three tiers displayed in the plugin store: unverified (community, no review), reviewed (community-reviewed, basic checks), official (maintained by the project)

**Deliverables**

- Plugin supply chain is secured against tampering
- Users can trust verified plugins and make informed decisions about unverified ones

**Dependencies**

- 4.1

---

### 4.3 — Multi-User / Household Support

**Scope**

Enable multiple users on a single Core instance:

- **Multiple user accounts** — each user authenticates via Pocket ID with their own identity
- **Per-user data isolation** — each user's data stored in a separate namespace, no cross-access by default
- **Shared collections** — designated collections shared between household members (e.g., family calendar, shopping list), configurable per collection
- **Role-based access** — three roles: admin (full control, user management), member (own data + shared collections), guest (read-only access to specific shared collections)
- **Household management UI** — invite users (email/link), assign roles, manage shared collections, view activity feed

**Deliverables**

- Families share a single Core instance with both isolated and shared data
- Each user has independent authentication and data ownership

**Dependencies**

- Phase 3

---

### 4.4 — GraphQL API Plugin

**Scope**

Provide a GraphQL API as an alternative to REST:

- **Auto-generated types** — generate GraphQL types from CDM JSON Schemas automatically
- **Operations** — queries, mutations, and subscriptions for all collections
- **Nested queries** — query across related collections (e.g., contacts with their associated emails and events)
- **Filtering, sorting, pagination** — full query capabilities matching the REST API
- **GraphQL Playground** — bundled interactive query explorer at `/api/graphql/playground`

**Deliverables**

- Power users and third-party apps can query Core via GraphQL
- Interactive playground available for exploration and testing

**Dependencies**

- Phase 3

---

### 4.5 — Federated Sync (Hub-to-Hub)

**Scope**

Enable peer-to-peer synchronisation between Core instances:

- **Federation protocol** — encrypted mTLS transport, selective sync (choose which collections to share), pull-based (receiving instance pulls from offering instance)
- **Federation API** — CRUD for peer management, manual and scheduled sync triggers, status view showing last sync time and record counts per peer
- **Use case** — share a calendar between partners' separate Core instances, each maintaining ownership of their own data

**Deliverables**

- Two Core instances can sync selected collections peer-to-peer

**Dependencies**

- Phase 3

---

### 4.6 — Encrypted Remote Backup

**Scope**

Implement backup and restore for disaster recovery:

- **Backup plugin** — full and incremental backups, encrypted with the master passphrase (same key derivation as SQLCipher), configurable targets: local filesystem, S3-compatible storage, WebDAV server
- **Scheduling** — configurable backup schedule (daily, weekly, custom cron), retention policy (keep N backups, age-based expiry)
- **Restore** — restore command with integrity verification, support full restore or partial restore (specific collections)

**Deliverables**

- Users back up and restore encrypted data to any supported storage target
- Backups are encrypted and verifiable

**Dependencies**

- Phase 3

---

### 4.7 — Identity and Credential System

**Scope**

Enable secure storage and selective disclosure of identity information:

- **Credential store** — encrypted storage for identity documents (passport, licence, certificates), separate encryption key from main data, CRUD API for managing credentials, credential values never appear in logs or API responses without explicit request
- **Selective disclosure** — generate signed tokens asserting specific facts (e.g., "over 18", "licensed driver") without revealing the raw document, time-limited tokens, audit log of all disclosures
- **Standards alignment** — W3C Verifiable Credentials 2.0 data format, DID (Decentralised Identifier) alignment for future interoperability

**Deliverables**

- Users store identity credentials and prove claims without sharing raw documents

**Dependencies**

- Phase 3

---

### 4.8 — Mobile Releases (iOS + Android)

**Scope**

Ship Life Engine on mobile platforms:

- **Tauri v2 mobile builds** — iOS (WKWebView), Android (WebView), shared codebase with desktop
- **Mobile-adapted UI** — bottom navigation (replacing sidebar), bottom sheets for detail views, touch-optimised input controls (larger tap targets, swipe gestures), pull-to-refresh for sync
- **Mobile constraints** — background sync (iOS background fetch, Android WorkManager), push notifications (APNs, FCM), battery optimisation (adaptive sync frequency)
- **App Store submissions** — Apple App Store ($99/yr developer account), Google Play Store ($25 one-time developer account)

**Deliverables**

- Life Engine available on the iOS App Store and Google Play Store

**Dependencies**

- Phase 3

---

## Exit Criteria

- WASM sandboxes enforce capability isolation for all Core plugins
- Plugin signing prevents tampered code from loading without explicit opt-in
- Multi-user households share a single Core instance with isolated and shared data
- GraphQL API available as an alternative to REST
- Federation works peer-to-peer between two Core instances
- Encrypted backups work to local, S3, and WebDAV targets
- Identity credentials stored and selectively disclosed using W3C VC format
- Mobile apps published on iOS App Store and Google Play Store

## Phase-Specific Risks

- **WASM performance overhead** — WASM execution adds overhead compared to native Rust plugins. Mitigation: benchmark all first-party plugins in both native and WASM modes, allow first-party plugins to remain native if performance is unacceptable for specific use cases.
- **Tauri v2 mobile maturity** — Tauri v2 mobile support is relatively new and may have platform-specific bugs. Mitigation: monitor Tauri mobile stability throughout earlier phases, maintain PWA as a fallback if native mobile proves unreliable.
- **Federation protocol design** — Designing a secure, efficient federation protocol is non-trivial and easy to over-engineer. Mitigation: keep the protocol minimal (selective collection sync only), expand based on real usage patterns rather than hypothetical needs.
- **App Store review for plugin-loading apps** — Both Apple and Google have policies around apps that load executable code at runtime. Mitigation: bundle all first-party plugins at submission time, document the plugin model clearly for reviewers, and ensure WASM plugins cannot access native APIs outside the declared sandbox.
