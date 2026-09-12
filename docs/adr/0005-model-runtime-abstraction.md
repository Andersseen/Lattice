# 0005 - Keep Model Runtime Replaceable

## Status

Accepted

## Context

Lattice expects llmster to be the initial local model runtime. Directly coupling UI or core product behavior to llmster would make later replacement or additional runtimes expensive.

## Decision

Treat model execution as a future `ModelRuntime` boundary. llmster should enter through an adapter when that capability is specified.

The `0.5` runtime discovery change introduces the first consumed management boundary. Lattice stores a user-selected absolute executable path and runs bounded read-only probes only when the user explicitly approves a probe action. The llmster adapter uses fixed `lms` CLI argument arrays for version, daemon status, and server status. Angular consumes neutral runtime discovery DTOs and does not call llmster, process APIs, network probes, SQL, or filesystem APIs directly.

Discovery status and approved executable metadata are persisted in Rust-owned SQLite storage. Executable identity is path/metadata based for this minor; changed identity requires a new explicit probe before the status is trusted again. Lifecycle ownership, start/stop, model inventory, model load/unload, and provider inference remain future minors.

Current LM Studio docs were checked on 2026-09-07: `lms daemon status --json` reports llmster running/not-running, `lms server status --json --quiet` reports server running/port, and the local server defaults to localhost port `1234`. The minimum recognized CLI version for 0.5 fixtures is `0.0.47`, matching the current CLI documentation example.

## Consequences

- No llmster API is exposed to Angular.
- The UI depends on Lattice runtime discovery DTOs, not llmster command output.
- 0.5 can report missing, unsupported, stopped, running, unreachable, and unknown states without mutating runtime resources.
- Future lifecycle work still needs its own OpenSpec proposal and must not assume that a discovered executable is owned by Lattice.
- 0.6 lifecycle ownership decisions (start/stop, owned/attached identity) are recorded separately in ADR 0010.
- 0.8's `Provider` transport/streaming decisions are recorded separately in ADR 0012.
