# ADR 0009: Use rusqlite For Local Settings Storage

## Status

Accepted for the `0.4` persistent application settings change.

## Context

The 0.4 roadmap minor needs validated non-secret settings, SQLite storage, migrations, backup behavior, and restart persistence before runtime configuration. Angular must not choose SQL statements or storage paths.

## Decision

Use `rusqlite 0.40.2` with the `bundled` feature inside Rust-owned settings storage. Tauri resolves the OS app-data directory, and `lattice-core` owns schema versioning, migrations, validation, revision conflicts, and safe storage errors.

`rusqlite 0.40.2` was checked against docs.rs/crates.io on 2026-09-07. The bundled feature vendors SQLite for the desktop app so storage behavior does not depend on an older or missing system SQLite library.

## Alternatives Considered

- JSON file storage: simpler, but does not meet the roadmap's SQLite and transactional migration requirements.
- Tauri SQL/plugin access: risks placing persistence authority near Angular, contrary to the architecture boundary.
- `sqlx` or an async pool: useful for larger concurrent services, but unnecessary for serialized local settings writes and adds runtime/build complexity.

## Consequences

The app gains one native dependency and a vendored SQLite build path. Runtime overhead remains bounded to one local connection guarded by desktop managed state, with no background worker or indexing service. Future data domains can reuse the migration approach when they have current consumers, but this ADR does not authorize speculative repositories.
