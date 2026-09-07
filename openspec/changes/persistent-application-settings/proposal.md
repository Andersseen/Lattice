# Persistent Application Settings

## Objective

Retain validated non-secret application preferences across restart through Rust-owned SQLite storage.

## Rationale

Runtime discovery and model lifecycle need safe local configuration before they can store executable identity or idle policies. The current foundation has a checked application IPC boundary but no persistence, migration policy, or settings UI.

## Scope

- Add Rust-owned `AppSettings` contracts and checked IPC commands for read, update, and reset.
- Store settings in a SQLite database under the OS app-data directory resolved by Tauri.
- Add transactional schema versioning, migration backup, newer-schema refusal, and safe storage errors.
- Validate update requests before persistence and reject stale revisions without overwriting existing settings.
- Add a minimal Settings UI for appearance and idle-unload preferences with loading, save, reset, error, and retry states.

## Non-goals

- No conversations, history, workspaces, model runtime, provider, credentials, cloud sync, filesystem tools, or universal repository framework.
- No secret values or credential references.
- No Angular access to SQL, storage paths, native filesystem APIs, or arbitrary persistence locations.

## Impacted Capabilities

- `application-api`
- `application-settings`
- `local-storage`
