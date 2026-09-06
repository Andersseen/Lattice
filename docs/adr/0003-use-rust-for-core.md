# 0003 - Use Rust For The Application Core

## Status

Accepted

## Context

Future Lattice capabilities include process management, model runtime lifecycle, storage, permissions, terminal and filesystem access, MCP transports, and credential handling. These are native and security-sensitive concerns.

## Decision

Use Rust for the application core and expose only typed, intentional APIs to Angular.

## Consequences

- Native capabilities remain centralized.
- Future adapters can evolve without exposing implementation details to the UI.
- Rust tests can cover core behavior independently from the webview.
- Contributors need to maintain Rust quality tooling from the start.
