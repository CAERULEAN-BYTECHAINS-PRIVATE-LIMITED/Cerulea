# Cerulea — Deep Architecture

Cerulea is a control-plane-driven infrastructure deployment system.

It is NOT frontend-first.

Architecture layers:

## Layer 1 — Protocol Layer
- consensus
- governance
- fee models
- runtime configuration
- EVM compatibility

## Layer 2 — Operations Layer
- deployment orchestration
- upgrade pipelines
- observability
- lifecycle hooks

## Layer 3 — Workflow Layer
- templates
- presets
- configuration flows
- chain specs
- runtime packaging

## Layer 4 — Interfaces
- Cerulea Studio
- dashboard
- CLI/SDK
- explorer
- wallet integrations

## Deployment model

Current:

- client-hosted runtime
- Cerulea orchestrates

Future:

- hybrid / Cerulea-hosted options

## Infra architecture decision

Cerulea follows hybrid control-plane architecture:

- central orchestration backend
- deployment engines
- infra connectors
- runtime nodes external

Not monolith.
Not pure serverless.
Not fragmented microservices.

## Non-blockchain infra

Currently supported.

Blockchain will become mandatory later.

## Artifact outputs

Cerulea generates:

- chain specs
- runtime config
- deployment orchestration artifacts
- operational config

Not application code.
