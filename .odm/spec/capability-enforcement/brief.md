<!--
domain: capability-enforcement
status: draft
tier: 1
updated: 2026-03-22
-->

# Capability Enforcement Spec

## Overview

This spec defines the capability system that governs plugin access to shell resources. Capabilities are declared in the plugin manifest, granted at install time with user approval, and enforced at runtime on every ShellAPI call. The system follows a deny-by-default model: plugins receive no access to any protected resource unless they declare the capability and the user approves it during installation.

## Goals

- Enforce deny-by-default access control for all plugin operations
- Present a clear install-time approval dialog listing all requested capabilities
- Perform synchronous runtime capability checks on every ShellAPI method with zero async overhead
- Scope capabilities to specific collections, domains, and IPC targets to prevent overly broad access
- Highlight high-trust capabilities (data:write, network:fetch) with visible warnings during installation
- Throw descriptive `CapabilityError` on unauthorized operations instead of silent failure

## User Stories

- As a user, I want to see what capabilities a plugin requests before installing it so that I can make an informed decision.
- As a plugin author, I want to declare capabilities in my manifest so that the shell grants me access to the resources I need.
- As a security-conscious user, I want high-trust capabilities highlighted with warnings so that I notice when a plugin requests data write or network access.
- As a developer, I want unauthorized operations to throw a clear `CapabilityError` so that I can debug missing capabilities quickly.

## Functional Requirements

- The system must check capabilities synchronously at the start of every ShellAPI method call before any work is performed.
- The system must throw a `CapabilityError` with plugin ID, attempted operation, and missing capability when a check fails.
- The system must present an install-time approval dialog listing all requested capabilities with human-readable descriptions.
- The system must scope data capabilities to specific collection names, network:fetch to allowedDomains, and ipc:send to target plugin IDs.
- The system must visually distinguish high-trust capabilities (data:write, network:fetch) in the install dialog.
- The system must re-prompt for approval only when a plugin update adds new capabilities.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
