<!--
domain: workflow-engine
status: draft
tier: 1
updated: 2026-03-22
-->

# Workflow Engine Spec

## Overview

This spec defines how Core chains plugins in sequence to process requests. The workflow engine is the primary way Core processes data: every request that involves plugin logic flows through a workflow. Workflows are API-managed and stored in the database, so no Core restart is needed to create, update, or delete them.

## Goals

- Provide a declarative way to define multi-step plugin pipelines triggered by API routes
- Pass typed data between steps with compatibility validation at workflow creation time
- Support configurable error strategies (halt, skip, retry) per step for resilient processing
- Log all workflow executions with full context for debugging and auditing
- Allow live workflow management via REST API without Core restarts

## User Stories

- As an admin, I want to define a workflow that chains email fetch, spam filter, and archiver plugins so that incoming email is processed automatically.
- As a developer, I want workflow creation to reject incompatible step pairs so that I catch data flow errors before runtime.
- As an admin, I want to choose halt, skip, or retry per step so that non-critical steps do not block the entire pipeline.
- As a developer, I want detailed execution logs for each workflow run so that I can diagnose failures quickly.
- As an admin, I want to update a workflow definition and have it take effect immediately so that I do not need to restart Core.

## Functional Requirements

- The system must expose CRUD endpoints at `/api/workflows` for workflow definitions.
- The system must execute workflow steps in order, passing the output of step N as input to step N+1.
- The system must validate type compatibility between adjacent steps at workflow creation time.
- The system must support `halt`, `skip`, and `retry` error strategies per step.
- The system must retry failed steps with exponential backoff up to a configurable maximum.
- The system must log every workflow execution with step-level detail including inputs, outputs, errors, and timestamps.

## Spec Files

- [Design](./design.md)
- [Requirements](./requirements.md)
- [Tasks](./tasks.md)
