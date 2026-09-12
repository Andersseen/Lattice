//! Low-level SQLite connection mechanics shared by every storage domain:
//! opening the connection file, reading the schema version pragma, and
//! validating the small scalar shapes (`revision`) that every domain's
//! rows carry. Schema content/evolution lives in `migrations`; what to do
//! with an open, current-schema connection lives in `settings`/`runtime`.

use crate::AppError;
use rusqlite::Connection;
use std::{fs, path::Path};

pub(super) fn open_connection(path: &Path) -> Result<Connection, AppError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|_| {
            AppError::storage_unavailable("Lattice could not prepare local settings storage.")
        })?;
    }

    Connection::open(path).map_err(|_| {
        AppError::storage_unavailable("Lattice could not open local settings storage.")
    })
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
