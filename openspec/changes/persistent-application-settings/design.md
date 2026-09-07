# Persistent Application Settings Design

## Ownership

`lattice-core` owns settings domain validation, revision transitions, storage errors, migrations, and the SQLite repository. `lattice-desktop` resolves the app-data directory with Tauri and exposes only typed commands. Angular owns presentation state and calls the existing typed application API.

## Contracts

`AppSettings` contains:

- `schemaVersion`: current settings schema version.
- `revision`: monotonically increasing revision for optimistic conflict checks.
- `appearance`: `system`, `light`, or `dark`.
- `idleUnloadMinutes`: integer from 1 to 120. The default is 5 minutes.

Commands:

- `get_app_settings` returns current settings, creating the database/default row through migrations if needed.
- `update_app_settings` accepts an expected revision and an optional patch for current 0.4 keys.
- `reset_app_settings` accepts an expected revision and restores defaults.

Invalid patches and stale revisions fail before writing. All storage errors are safe `AppError` values; raw SQL/path/internal details stay Rust-side.

## Storage And Migrations

The desktop shell creates `lattice.sqlite3` under Tauri's app-data directory and opens it through `lattice-core::SettingsStore`. SQLite schema version is stored in `PRAGMA user_version`.

Opening a schema newer than the current binary returns `storage.unsupported_schema` and never rewrites data. A pre-migration copy is written beside the database before migrating an existing non-empty file. Migrations run in a transaction; failed migration leaves the original database transaction state intact and keeps the backup copy when one was created.

0.4 uses one table, `app_settings`, with a single row (`id = 1`) and check constraints for persisted values. There is no SQL IPC and no user-supplied storage path.

## Dependency Admission

Chosen dependency: `rusqlite 0.40.2` with the `bundled` feature, verified against docs.rs/crates.io on 2026-09-07.

Current consumer: 0.4 settings persistence. Alternative considered: JSON file storage would not satisfy the roadmap's SQLite/migration requirement; Tauri SQL/plugin access would put persistence too close to Angular; `sqlx`/async pools add runtime and build cost not needed for serialized local settings writes.

Runtime cost: one SQLite connection guarded by desktop managed state, opened at app startup; no background worker, polling, indexing, or resident helper service. Build cost includes vendored SQLite through `libsqlite3-sys`; this avoids relying on a platform SQLite version for a desktop app that controls its own database.

Maintenance/license: `rusqlite` is MIT, documented and actively maintained. Bundled SQLite is public domain. Removal path is limited to the settings repository adapter because Angular consumes only generated DTOs and IPC commands.

## UI

The Settings route reads settings on startup, applies the appearance preference, and exposes segmented appearance controls, an idle-unload numeric control, Save, Reset, and Retry. Browser smoke mode uses an in-memory fallback only; persistence claims apply to native SQLite.

## Verification

Focused verification covers temporary databases, invalid values, revision conflicts, rollback/newer-schema behavior, command permissions, generated binding drift, Angular decoding, and UI states. Native restart evidence is recorded separately when a desktop shell is available; browser E2E is not labeled as native persistence evidence.
