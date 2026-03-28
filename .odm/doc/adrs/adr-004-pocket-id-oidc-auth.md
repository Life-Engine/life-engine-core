---
title: "ADR-004: Pocket ID for OIDC Authentication"
type: adr
created: 2026-03-27
status: active
---

# ADR-004: Pocket ID for OIDC Authentication

## Status

Accepted

## Context

Life Engine is a self-hosted platform. Every request to Core's API must be authenticated. The authentication system must:

- Work entirely without cloud services. Users who self-host Life Engine should not depend on Auth0, Okta, or any external identity provider.
- Implement a standard protocol (OIDC/OAuth 2.0) so that third-party integrations can authenticate against Core using industry-standard flows.
- Be deployable alongside Core with minimal configuration. Self-hosters are not professional sysadmins; the auth sidecar must be simple to bring up with Docker Compose.
- Support multiple users on a single Core instance (family or small team use cases planned for Phase 2).
- Provide a user management UI that non-technical users can operate for adding or removing accounts.

API key authentication is also required for server-to-server integrations (e.g., a cron job or home automation system calling Core). API keys must go through the same middleware as OIDC tokens; they are not a bypass of authentication.

The authentication layer must be a separately deployable process, not embedded in Core, so that it can be upgraded independently and so that Core's attack surface does not include the auth implementation.

## Decision

Pocket ID is used as the OIDC identity provider. Pocket ID is a self-hosted, lightweight OIDC server designed specifically for use as a sidecar identity provider for self-hosted applications. It is packaged as a single binary (or Docker image), requires only a SQLite database, and exposes the standard OIDC Discovery endpoint.

Core validates OIDC tokens issued by Pocket ID using the JWKS endpoint for public key verification. API keys are stored as hashed values in Core's own database and checked in the same auth middleware as OIDC tokens. Core never stores plaintext passwords; all password-based authentication flows go through Pocket ID.

## Consequences

Positive consequences:

- Pocket ID's minimal footprint (single binary, SQLite) matches the self-hosting ethos. The Docker Compose dev environment brings it up in one command.
- Standard OIDC means Core's auth middleware validates tokens against a well-known specification, not a proprietary API.
- Pocket ID's user management UI allows non-technical self-hosters to manage accounts without touching configuration files.
- Pocket ID can be updated independently of Core. A security fix in the auth sidecar does not require a Core release.
- Standard OIDC enables future integration with other identity providers (corporate SSO, Authelia, etc.) without changing Core's auth middleware — only the `OIDC_ISSUER_URL` config value changes.
- The separation of identity (Pocket ID) from data (Core) is an application of Defence in Depth: a vulnerability in Core's API does not automatically compromise the authentication service.

Negative consequences:

- Pocket ID is a newer and smaller project than Keycloak or Authelia. Its feature set is limited by design (no SAML, no complex federation). Advanced use cases may require replacing it in Phase 3.
- Running a separate identity sidecar increases operational complexity for self-hosters compared to embedded auth. Users must manage two processes instead of one.
- Pocket ID's community and documentation are smaller than Keycloak's. Contributors debugging OIDC flow issues may find fewer Stack Overflow answers.
- The OIDC token validation in Core requires network availability of Pocket ID's JWKS endpoint at startup (for key fetching) and on validation failures. If Pocket ID is down, Core cannot validate new sessions.

## Alternatives Considered

**Keycloak** is the most feature-complete self-hosted identity provider. It was rejected because it is a Java-based application requiring a JVM, a PostgreSQL database, and significant memory (512MB minimum recommended). This is too heavy for the target deployment environment (home server, Raspberry Pi). Keycloak's configuration complexity also conflicts with the goal of a simple setup for non-technical self-hosters.

**Custom authentication** (implementing OIDC or a proprietary session system inside Core itself) was considered. It was rejected because implementing authentication correctly is a high-risk endeavour. Rolling a custom auth system is a known source of security vulnerabilities. The Principle of Least Privilege applies to the development process too: Core should not be responsible for implementing authentication.

**Authelia** is a self-hosted authentication and authorisation server with multi-factor authentication support. It was considered but rejected because it is primarily designed as a reverse-proxy authentication layer (protecting whole applications, not individual API endpoints). Integrating it as a proper OIDC provider for Core's token validation model requires more configuration than Pocket ID. Authelia is a viable future replacement if advanced MFA or proxy-auth use cases arise in Phase 3.
