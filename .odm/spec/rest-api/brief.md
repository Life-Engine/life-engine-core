<!--
domain: core
status: draft
tier: 1
updated: 2026-03-22
-->

# REST API Spec

## Overview

This spec defines all HTTP routes, the middleware stack, request/response conventions, and real-time event streaming for the Core REST API. The API is the only way clients interact with Core. It is implemented in `axum` and defined in a shared `packages/api` crate so any client (App, web, mobile, CLI) can consume it without Core changes.

## Goals

- Single entry point — All client interaction goes through HTTP; no side channels.
- Consistency — Every route follows the same request/response conventions and error shape.
- Real-time — SSE stream delivers plugin events, workflow results, and scheduler updates live.
- Extensibility — Plugin routes mount dynamically under a namespace without Core changes.

## User Stories

- As a client app, I want CRUD endpoints for all collections so that I can read and write data.
- As a plugin, I want my routes mounted under my namespace so that clients can call my endpoints.
- As a frontend developer, I want an SSE stream so that I can show real-time updates without polling.
- As an operator, I want a health check endpoint so that I can monitor Core subsystem status.
- As a security auditor, I want consistent error shapes so that no internal details leak to clients.

## Functional Requirements

- The system must apply middleware in order: TLS, auth, rate limiting, CORS, logging, error handling.
- The system must expose data CRUD routes at `/api/data/{collection}`.
- The system must expose plugin management routes at `/api/plugins`.
- The system must expose workflow CRUD routes at `/api/workflows`.
- The system must expose scheduler routes at `/api/scheduler`.
- The system must expose credential routes at `/api/credentials`.
- The system must expose system routes at `/api/system` including health check.
- The system must expose auth routes at `/api/auth`.
- The system must provide an SSE endpoint at `/api/events/stream`.
- The system must return consistent error responses as `{ "error": { "code", "message" } }`.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
