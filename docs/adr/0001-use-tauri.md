# 0001 - Use Tauri For Desktop

## Status

Accepted

## Context

Lattice is a desktop application intended to stay lightweight, local-first, and native-capability aware. Electron would provide a familiar web stack but adds a persistent Chromium/Node footprint that works against the memory goals for machines with 16 GB unified memory.

## Decision

Use Tauri 2 for the desktop shell.

## Consequences

- Rust owns native capabilities and security-sensitive boundaries.
- The frontend can stay web-based without shipping a persistent Node runtime.
- Contributors need Rust and Tauri platform prerequisites.
- Native desktop automation is more complex than browser-only E2E and is deferred until needed.
