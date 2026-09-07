# Application Runtime Foundation

## Objective

Make the existing Angular-to-Tauri application boundary deterministic, decoded at runtime, and resistant to TypeScript/Rust contract drift.

## Rationale

The foundation currently has one native metadata command, but its TypeScript DTO is hand-written and Angular trusts the generic `invoke<T>()` type assertion. Later commands will multiply this risk unless the repo first establishes one checked wire contract, one error normalizer, and one command inventory.

## Scope

- Generate committed TypeScript wire bindings from Rust-owned application contract metadata.
- Use the generated command inventory in the Angular application API.
- Decode untrusted IPC responses before returning app data to feature state.
- Normalize unknown and malformed bridge errors through one safe `AppError` path.
- Prevent stale metadata refreshes from overwriting newer refresh results.
- Enable strict Angular template checking and TypeScript checking for spec sources.

## Non-goals

- No settings, SQLite, process lifecycle, model runtime, provider, MCP, filesystem, terminal, memory, Space, task, or scheduling behavior.
- No UI redesign.
- No dependency admission unless a current consumer needs it.

## Impacted Capabilities

- `foundation`
- `application-api`
