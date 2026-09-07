# Model Runtime Discovery

## Objective

Report whether a supported llmster/LM Studio CLI installation and local endpoint are available without starting, stopping, downloading, or loading anything.

## Rationale

0.5 is the first real `ModelRuntime` consumer. Lifecycle and model management need an approved executable identity, honest unavailable states, and persisted discovery metadata before any owned process control is added.

## Scope

- Add Rust-owned ModelRuntime discovery DTOs and checked IPC commands.
- Let the user configure an absolute runtime executable path without probing it.
- Run bounded read-only CLI probes only through an explicit probe command.
- Record executable identity, CLI version, daemon observation, server observation, endpoint health, and last checked time in local SQLite.
- Add a minimal Models/setup UI that consumes runtime discovery status.

## Non-goals

- No daemon/server start or stop.
- No model list, model load/unload, model download, provider inference, chat, streaming, credentials, or lifecycle ownership.
- No executable lookup from project directories or `$PATH`.
- No Angular access to process APIs, network probes, SQL, or storage paths.

## Impacted Capabilities

- `application-api`
- `application-settings`
- `local-storage`
- `model-runtime`
