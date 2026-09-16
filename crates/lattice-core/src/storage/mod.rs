//! Rust-owned local persistence, split by concern rather than kept as one
//! growing file: `database` (connection/version/scalar mechanics),
//! `migrations` (schema evolution), `settings` (non-secret application
//! preferences) and `runtime` (`ModelRuntime`/local-model state). Those
//! four share one [`SettingsStore`] handle and one SQLite file — the
//! storage contract 0.4 established — but each owns only its own table(s)
//! and its own slice of `SettingsStore`'s methods, via a separate `impl
//! SettingsStore` block per concern.
//!
//! `conversations` (0.9), `credentials` (0.10) and `provider_profiles`
//! (0.11) are the domains that follow this module's own prior guidance
//! literally: each is a sibling module with its own store type
//! ([`conversations::ConversationStore`], [`credentials::CredentialStore`],
//! [`provider_profiles::ProviderProfileStore`]) behind its own IPC surface — not
//! another `impl SettingsStore` block. They still share the same SQLite
//! file and the same versioned `migrate()` cascade (each its own
//! independent `rusqlite::Connection` to that file; see their module doc
//! comments for why), so `CURRENT_SCHEMA_VERSION` remains one number shared
//! by every domain. A later domain (memory, tasks) should follow their
//! shape, not `settings`/`runtime`'s.

mod conversations;
mod credentials;
mod database;
mod migrations;
mod provider_profiles;
mod runtime;
mod settings;
#[cfg(test)]
mod test_support;

pub use conversations::ConversationStore;
pub use credentials::CredentialStore;
pub use provider_profiles::ProviderProfileStore;
pub use settings::{
    AppSettings, AppearancePreference, ResetAppSettingsRequest, UpdateAppSettingsRequest,
    GET_APP_SETTINGS_COMMAND, RESET_APP_SETTINGS_COMMAND, UPDATE_APP_SETTINGS_COMMAND,
};

use crate::AppError;
use rusqlite::Connection;
use std::path::Path;

pub(crate) const CURRENT_SCHEMA_VERSION: u32 = 7;
pub(crate) const MODEL_LOAD_ROW_ID: i64 = 1;

pub struct SettingsStore {
    conn: Connection,
}

impl SettingsStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref();
        let mut conn = database::open_connection(path)?;
        migrations::migrate(&mut conn, Some(path))?;

        Ok(Self { conn })
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self, AppError> {
        let mut conn = Connection::open_in_memory().map_err(|_| {
            AppError::storage_unavailable("Lattice could not open local settings storage.")
        })?;
        database::apply_connection_pragmas(&conn)?;
        migrations::migrate(&mut conn, None)?;
        Ok(Self { conn })
    }
}

#[cfg(test)]
mod tests {
    use super::SettingsStore;
    use std::{error::Error, fs};
    use tempfile::tempdir;

    #[test]
    fn storage_path_must_be_preparable() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let not_a_directory = directory.path().join("settings-parent");
        fs::write(&not_a_directory, b"not a directory")?;

        let error = SettingsStore::open(not_a_directory.join("lattice.sqlite3"))
            .err()
            .ok_or("expected storage preparation error")?;

        assert_eq!(error.code, "storage.unavailable");
        Ok(())
    }
}
