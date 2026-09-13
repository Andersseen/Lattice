//! Low-level SQLite connection mechanics shared by every storage domain:
//! opening the connection file, reading the schema version pragma, and
//! validating the small scalar shapes (`revision`) that every domain's
//! rows carry. Schema content/evolution lives in `migrations`; what to do
//! with an open, current-schema connection lives in `settings`/`runtime`.

use crate::AppError;
use rusqlite::Connection;
use std::{fs, path::Path};

/// Milliseconds SQLite will retry an internally-locked write before
/// returning `SQLITE_BUSY`. Needed starting with 0.9: `SettingsStore` and
/// `ConversationStore` are two same-process `rusqlite::Connection`s opened
/// against the same file (see `storage/conversations.rs`'s module doc
/// comment), and SQLite's own default busy timeout is `0` (immediate
/// failure on any lock collision). Settings and conversation writes are
/// always disjoint tables in disjoint transactions and rare relative to
/// read traffic, so this only smooths over the rare case where two writes
/// land in the same instant rather than papering over real contention.
const BUSY_TIMEOUT_MILLISECONDS: u32 = 5_000;

pub(super) fn open_connection(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            AppError::storage_unavailable("Lattice could not prepare local settings storage.")
        })?;
    }

    let conn = Connection::open(path).map_err(|_| {
        AppError::storage_unavailable("Lattice could not open local settings storage.")
    })?;
    apply_connection_pragmas(&conn)?;
    Ok(conn)
}

/// Applied to every connection, in-memory test fixtures included, so
/// storage behavior does not depend on which store happened to open first.
/// `foreign_keys` defaults to off in SQLite; the conversations schema's
/// `messages.conversation_id ... ON DELETE CASCADE` (0.9) relies on it
/// being on for `delete_conversation` to actually remove a conversation's
/// messages.
pub(super) fn apply_connection_pragmas(conn: &Connection) -> Result<(), AppError> {
    conn.pragma_update(None, "busy_timeout", BUSY_TIMEOUT_MILLISECONDS)
        .map_err(|_| AppError::storage_unavailable("Lattice could not configure local storage."))?;
    conn.pragma_update(None, "foreign_keys", true)
        .map_err(|_| AppError::storage_unavailable("Lattice could not configure local storage."))?;
    Ok(())
}

pub(super) fn schema_version(conn: &Connection) -> Result<u32, AppError> {
    let raw = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, i64>(0))
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))?;

    u32::try_from(raw)
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))
}

/// Shared by every domain's row decoding: a stored `revision`/timestamp
/// column is always a positive SQLite `INTEGER`, never a client-supplied
/// value, so a non-positive or overflowing value indicates corrupted or
/// foreign storage rather than a user-facing validation error.
pub(super) fn validate_revision(value: i64) -> Result<u64, AppError> {
    if value < 1 {
        return Err(AppError::storage_unavailable(
            "Lattice could not read local settings.",
        ));
    }

    u64::try_from(value)
        .map_err(|_| AppError::storage_unavailable("Lattice could not read local settings."))
}
