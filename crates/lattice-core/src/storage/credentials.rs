//! The 0.10 credentials repository: a sibling store type to
//! [`super::SettingsStore`]/[`super::ConversationStore`], not another
//! `impl SettingsStore` block, per `storage/mod.rs`'s own doc comment.
//! `CredentialStore` opens a third, independent `rusqlite::Connection` to
//! the *same* `lattice.sqlite3` file — same `busy_timeout`/`foreign_keys`
//! pragmas and versioned `migrate()` cascade `ConversationStore` already
//! established. Only reference metadata (label, provider key, timestamps)
//! lives here; the secret itself never touches this file — it lives
//! exclusively behind the injected [`SecretStore`], resolved through
//! `crate::credentials::new_secret_store()` in production (the real OS
//! keychain on macOS, a fail-closed stub everywhere else).

use super::{database, migrations};
use crate::credentials::{
    new_credential_id, unix_timestamp_now, validate_label, validate_provider_key,
    CreateCredentialRequest, CredentialAvailability, CredentialRef, DeleteCredentialRequest,
    PromptOutcome, ReplaceCredentialRequest, SecretPrompt, SecretStore, SecretStoreError,
};
use crate::AppError;
use rusqlite::{params, Connection, OptionalExtension};
use std::path::Path;

pub struct CredentialStore {
    conn: Connection,
    secret_store: Box<dyn SecretStore>,
}

impl CredentialStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        Self::open_with_store(path, crate::credentials::new_secret_store())
    }

    fn open_with_store(
        path: impl AsRef<Path>,
        secret_store: Box<dyn SecretStore>,
    ) -> Result<Self, AppError> {
        let path = path.as_ref();
        let mut conn = database::open_connection(path)?;
        migrations::migrate(&mut conn, Some(path))?;
        Ok(Self { conn, secret_store })
    }

    #[cfg(test)]
    fn open_in_memory_with_store(secret_store: Box<dyn SecretStore>) -> Result<Self, AppError> {
        let mut conn = Connection::open_in_memory().map_err(|_| {
            AppError::storage_unavailable("Lattice could not open local credential storage.")
        })?;
        database::apply_connection_pragmas(&conn)?;
        migrations::migrate(&mut conn, None)?;
        Ok(Self { conn, secret_store })
    }

    /// Lists credentials most-recently-updated first. Calls
    /// `secret_store.get` once per row to derive an honest availability —
    /// there is no cheaper keychain-wide "is locked" query in the
    /// underlying API (see design.md), so accuracy per row is preferred
    /// over a single shared guess.
    pub fn list(&self) -> Result<Vec<CredentialRef>, AppError> {
        let mut statement = self
            .conn
            .prepare(
                "SELECT id, label, provider_key, created_at_unix_seconds, updated_at_unix_seconds
                 FROM credentials
                 ORDER BY updated_at_unix_seconds DESC, id DESC",
            )
            .map_err(|_| AppError::storage_unavailable("Lattice could not list credentials."))?;

        let rows = statement
            .query_map([], |row| {
                Ok(RawCredentialRow {
                    id: row.get(0)?,
                    label: row.get(1)?,
                    provider_key: row.get(2)?,
                    created_at_unix_seconds: row.get(3)?,
                    updated_at_unix_seconds: row.get(4)?,
                })
            })
            .map_err(|_| AppError::storage_unavailable("Lattice could not list credentials."))?;

        let mut refs = Vec::new();
        for row in rows {
            let row = row.map_err(|_| {
                AppError::storage_unavailable("Lattice could not list credentials.")
            })?;
            let availability = self.availability_for(&row.id);
            refs.push(CredentialRef {
                id: row.id,
                label: row.label,
                provider_key: row.provider_key,
                availability,
                created_at_unix_seconds: nonneg_u64(row.created_at_unix_seconds),
                updated_at_unix_seconds: nonneg_u64(row.updated_at_unix_seconds),
            });
        }
        Ok(refs)
    }

    fn availability_for(&self, id: &str) -> CredentialAvailability {
        match self.secret_store.get(id) {
            Ok(_) => CredentialAvailability::Available,
            Err(SecretStoreError::NotFound) => CredentialAvailability::Missing,
            Err(SecretStoreError::Unsupported) => CredentialAvailability::Unsupported,
            Err(SecretStoreError::Locked | SecretStoreError::Failed) => {
                CredentialAvailability::Locked
            }
        }
    }

    /// Runs native entry through the OS-appropriate prompt (real on macOS,
    /// fail-closed elsewhere), exactly as `open` already defaults
    /// `secret_store` — the Tauri command layer never chooses or
    /// constructs a prompt itself.
    pub fn create(&mut self, request: CreateCredentialRequest) -> Result<CredentialRef, AppError> {
        self.create_with_prompt(request, crate::credentials::new_secret_prompt().as_ref())
    }

    /// Runs native entry, then writes the keychain entry before the SQLite
    /// row — if the row insert then fails, the keychain entry is deleted
    /// again so no orphaned secret survives a failed reference (spec: "A
    /// Keychain Write Failure Leaves No Orphaned Reference"). Takes an
    /// explicit prompt so tests can inject `FakeSecretPrompt`; `create`
    /// above is the production entry point.
    fn create_with_prompt(
        &mut self,
        request: CreateCredentialRequest,
        prompt: &dyn SecretPrompt,
    ) -> Result<CredentialRef, AppError> {
        let label = validate_label(&request.label)?;
        let provider_key = validate_provider_key(&request.provider_key)?;
        let secret = entered_secret(prompt)?;

        let id = new_credential_id();
        self.secret_store
            .set(&id, &secret)
            .map_err(map_store_error_for_write)?;

        let now = unix_timestamp_now();
        let insert_result = self.conn.execute(
            "INSERT INTO credentials (id, label, provider_key, created_at_unix_seconds, updated_at_unix_seconds)
             VALUES (?1, ?2, ?3, ?4, ?4)",
            params![id, label, provider_key, i64_or_zero(now)],
        );

        if insert_result.is_err() {
            let _ = self.secret_store.delete(&id);
            return Err(AppError::storage_unavailable(
                "Lattice could not save the credential.",
            ));
        }

        Ok(CredentialRef {
            id,
            label,
            provider_key,
            availability: CredentialAvailability::Available,
            created_at_unix_seconds: now,
            updated_at_unix_seconds: now,
        })
    }

    /// Production entry point — see `create`'s equivalent note.
    pub fn replace(
        &mut self,
        request: ReplaceCredentialRequest,
    ) -> Result<CredentialRef, AppError> {
        self.replace_with_prompt(request, crate::credentials::new_secret_prompt().as_ref())
    }

    /// Re-runs native entry against an existing reference; only bumps
    /// `updated_at_unix_seconds` on success. Refuses an unknown id before
    /// ever prompting. Takes an explicit prompt so tests can inject
    /// `FakeSecretPrompt`.
    fn replace_with_prompt(
        &mut self,
        request: ReplaceCredentialRequest,
        prompt: &dyn SecretPrompt,
    ) -> Result<CredentialRef, AppError> {
        let existing = self.read_row(&request.id)?;
        let secret = entered_secret(prompt)?;

        self.secret_store
            .set(&request.id, &secret)
            .map_err(map_store_error_for_write)?;

        let now = unix_timestamp_now();
        self.conn
            .execute(
                "UPDATE credentials SET updated_at_unix_seconds = ?1 WHERE id = ?2",
                params![i64_or_zero(now), request.id],
            )
            .map_err(|_| AppError::storage_unavailable("Lattice could not save the credential."))?;

        Ok(CredentialRef {
            id: request.id,
            label: existing.label,
            provider_key: existing.provider_key,
            availability: CredentialAvailability::Available,
            created_at_unix_seconds: nonneg_u64(existing.created_at_unix_seconds),
            updated_at_unix_seconds: now,
        })
    }

    /// Deletes the reference, then best-effort deletes the keychain entry
    /// — an already-absent keychain entry at this point is not an error,
    /// the reference is gone either way.
    pub fn delete(&mut self, request: DeleteCredentialRequest) -> Result<(), AppError> {
        let changed = self
            .conn
            .execute("DELETE FROM credentials WHERE id = ?1", params![request.id])
            .map_err(|_| {
                AppError::storage_unavailable("Lattice could not delete the credential.")
            })?;

        if changed == 0 {
            return Err(AppError::credential_not_found(
                "That credential no longer exists.",
            ));
        }

        let _ = self.secret_store.delete(&request.id);
        Ok(())
    }

    /// Rust-internal only — deliberately not wired to any Tauri command.
    /// 0.11+ providers call this directly from `lattice-core`.
    pub fn resolve_secret(&self, id: &str) -> Result<Vec<u8>, AppError> {
        let exists: bool = self
            .conn
            .query_row(
                "SELECT EXISTS(SELECT 1 FROM credentials WHERE id = ?1)",
                params![id],
                |row| row.get(0),
            )
            .map_err(|_| AppError::storage_unavailable("Lattice could not read the credential."))?;
        if !exists {
            return Err(AppError::credential_not_found(
                "That credential no longer exists.",
            ));
        }

        self.secret_store.get(id).map_err(map_store_error_for_read)
    }

    fn read_row(&self, id: &str) -> Result<RawCredentialRow, AppError> {
        self.conn
            .query_row(
                "SELECT id, label, provider_key, created_at_unix_seconds, updated_at_unix_seconds
                 FROM credentials WHERE id = ?1",
                params![id],
                |row| {
                    Ok(RawCredentialRow {
                        id: row.get(0)?,
                        label: row.get(1)?,
                        provider_key: row.get(2)?,
                        created_at_unix_seconds: row.get(3)?,
                        updated_at_unix_seconds: row.get(4)?,
                    })
                },
            )
            .optional()
            .map_err(|_| AppError::storage_unavailable("Lattice could not read the credential."))?
            .ok_or_else(|| AppError::credential_not_found("That credential no longer exists."))
    }
}

/// Runs the prompt and maps `Cancelled`/an empty entry to
/// `AppError::credential_cancelled` — both mean nothing gets saved.
fn entered_secret(prompt: &dyn SecretPrompt) -> Result<Vec<u8>, AppError> {
    match prompt.prompt()? {
        PromptOutcome::Entered(secret) if !secret.is_empty() => Ok(secret),
        PromptOutcome::Entered(_) => Err(AppError::credential_cancelled("No value was entered.")),
        PromptOutcome::Cancelled => Err(AppError::credential_cancelled(
            "Credential entry was cancelled.",
        )),
    }
}

fn map_store_error_for_write(error: SecretStoreError) -> AppError {
    match error {
        SecretStoreError::Unsupported => AppError::credential_unsupported(
            "Secure credential storage is not supported on this platform.",
        ),
        SecretStoreError::Locked => {
            AppError::credential_unavailable("Unlock the keychain and try again.")
        }
        SecretStoreError::Failed | SecretStoreError::NotFound => {
            AppError::credential_unavailable("Lattice could not save the credential.")
        }
    }
}

fn map_store_error_for_read(error: SecretStoreError) -> AppError {
    match error {
        SecretStoreError::NotFound => {
            AppError::credential_not_found("That credential's secret is missing.")
        }
        SecretStoreError::Unsupported => AppError::credential_unsupported(
            "Secure credential storage is not supported on this platform.",
        ),
        SecretStoreError::Locked | SecretStoreError::Failed => {
            AppError::credential_unavailable("Unlock the keychain and try again.")
        }
    }
}

/// Read-validation for `migrations::migrate`'s "schema already current"
/// branch, mirroring `settings::read_settings`/`conversations::read_conversations_sanity`.
pub(super) fn read_credentials_sanity(conn: &Connection) -> Result<(), AppError> {
    conn.query_row("SELECT COUNT(*) FROM credentials", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|_| AppError::storage_unavailable("Lattice could not read local credentials."))?;
    Ok(())
}

fn nonneg_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn i64_or_zero(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(0)
}

struct RawCredentialRow {
    id: String,
    label: String,
    provider_key: String,
    created_at_unix_seconds: i64,
    updated_at_unix_seconds: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::credentials::{FakeSecretPrompt, FakeSecretStore};
    use std::error::Error;
    use tempfile::tempdir;

    fn store_with_fake() -> Result<(CredentialStore, std::sync::Arc<FakeSecretStore>), AppError> {
        let fake = std::sync::Arc::new(FakeSecretStore::new());
        let store =
            CredentialStore::open_in_memory_with_store(Box::new(FakeStoreHandle(fake.clone())))?;
        Ok((store, fake))
    }

    /// A thin `Box<dyn SecretStore>` wrapper delegating to a shared `Arc`,
    /// so tests can both hand ownership to `CredentialStore` and keep a
    /// handle to inspect/mutate the fake afterward.
    struct FakeStoreHandle(std::sync::Arc<FakeSecretStore>);

    impl SecretStore for FakeStoreHandle {
        fn set(&self, account: &str, secret: &[u8]) -> Result<(), SecretStoreError> {
            self.0.set(account, secret)
        }
        fn get(&self, account: &str) -> Result<Vec<u8>, SecretStoreError> {
            self.0.get(account)
        }
        fn delete(&self, account: &str) -> Result<(), SecretStoreError> {
            self.0.delete(account)
        }
    }

    fn create_request(label: &str, provider_key: &str) -> CreateCredentialRequest {
        CreateCredentialRequest {
            label: label.to_string(),
            provider_key: provider_key.to_string(),
        }
    }

    #[test]
    fn creates_a_credential_and_lists_it_as_available() -> Result<(), Box<dyn Error>> {
        let (mut store, fake) = store_with_fake()?;
        let created = store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"sk-test"),
        )?;

        assert_eq!(created.label, "OpenAI");
        assert_eq!(created.provider_key, "openai");
        assert_eq!(created.availability, CredentialAvailability::Available);
        assert!(fake.contains(&created.id));

        let listed = store.list()?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].availability, CredentialAvailability::Available);
        Ok(())
    }

    #[test]
    fn cancelling_entry_creates_nothing() -> Result<(), Box<dyn Error>> {
        let (mut store, _fake) = store_with_fake()?;
        let error = store
            .create_with_prompt(
                create_request("OpenAI", "openai"),
                &FakeSecretPrompt::cancelling(),
            )
            .err()
            .ok_or("expected cancellation error")?;

        assert_eq!(error.code, "credential.cancelled");
        assert_eq!(store.list()?.len(), 0);
        Ok(())
    }

    #[test]
    fn a_keychain_write_failure_leaves_no_orphaned_reference() -> Result<(), Box<dyn Error>> {
        let (mut store, fake) = store_with_fake()?;
        fake.fail_next_set();

        let error = store
            .create_with_prompt(
                create_request("OpenAI", "openai"),
                &FakeSecretPrompt::entering(b"sk-test"),
            )
            .err()
            .ok_or("expected a store failure")?;

        assert_eq!(error.code, "credential.unavailable");
        assert_eq!(store.list()?.len(), 0);
        Ok(())
    }

    #[test]
    fn replacing_an_unknown_credential_is_not_found() -> Result<(), Box<dyn Error>> {
        let (mut store, _fake) = store_with_fake()?;
        let error = store
            .replace_with_prompt(
                ReplaceCredentialRequest {
                    id: "missing".to_string(),
                },
                &FakeSecretPrompt::entering(b"new-value"),
            )
            .err()
            .ok_or("expected not-found error")?;
        assert_eq!(error.code, "credential.not_found");
        Ok(())
    }

    #[test]
    fn replace_updates_the_secret_and_bumps_updated_at() -> Result<(), Box<dyn Error>> {
        let (mut store, fake) = store_with_fake()?;
        let created = store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"old-value"),
        )?;

        let replaced = store.replace_with_prompt(
            ReplaceCredentialRequest {
                id: created.id.clone(),
            },
            &FakeSecretPrompt::entering(b"new-value"),
        )?;

        assert_eq!(replaced.id, created.id);
        assert_eq!(replaced.label, "OpenAI");
        assert!(replaced.updated_at_unix_seconds >= created.updated_at_unix_seconds);
        assert_eq!(
            fake.get(&created.id).map_err(|_| "expected secret")?,
            b"new-value"
        );
        Ok(())
    }

    #[test]
    fn deleting_removes_both_the_reference_and_the_keychain_entry() -> Result<(), Box<dyn Error>> {
        let (mut store, fake) = store_with_fake()?;
        let created = store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"sk-test"),
        )?;

        store.delete(DeleteCredentialRequest {
            id: created.id.clone(),
        })?;

        assert_eq!(store.list()?.len(), 0);
        assert!(!fake.contains(&created.id));
        Ok(())
    }

    #[test]
    fn deleting_an_unknown_credential_is_not_found() -> Result<(), Box<dyn Error>> {
        let (mut store, _fake) = store_with_fake()?;
        let error = store
            .delete(DeleteCredentialRequest {
                id: "missing".to_string(),
            })
            .err()
            .ok_or("expected not-found error")?;
        assert_eq!(error.code, "credential.not_found");
        Ok(())
    }

    #[test]
    fn resolve_secret_returns_the_stored_value_for_rust_internal_use() -> Result<(), Box<dyn Error>>
    {
        let (mut store, _fake) = store_with_fake()?;
        let created = store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"sk-test"),
        )?;

        assert_eq!(store.resolve_secret(&created.id)?, b"sk-test");
        Ok(())
    }

    #[test]
    fn resolve_secret_refuses_an_unknown_id() -> Result<(), Box<dyn Error>> {
        let (store, _fake) = store_with_fake()?;
        let error = store
            .resolve_secret("missing")
            .err()
            .ok_or("expected not-found error")?;
        assert_eq!(error.code, "credential.not_found");
        Ok(())
    }

    #[test]
    fn a_locked_keychain_is_reported_for_every_credential_not_missing() -> Result<(), Box<dyn Error>>
    {
        let (mut store, fake) = store_with_fake()?;
        let created = store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"sk-test"),
        )?;

        fake.set_locked(true);
        let listed = store.list()?;
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, created.id);
        assert_eq!(listed[0].availability, CredentialAvailability::Locked);
        Ok(())
    }

    #[test]
    fn a_keychain_entry_removed_outside_lattice_is_reported_as_missing(
    ) -> Result<(), Box<dyn Error>> {
        let (mut store, fake) = store_with_fake()?;
        let created = store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"sk-test"),
        )?;

        // Simulate external removal: delete directly from the fake
        // keychain without going through `CredentialStore::delete`, so the
        // SQLite reference row survives but the keychain entry does not.
        fake.delete(&created.id)
            .map_err(|_| "expected delete to succeed")?;

        let listed = store.list()?;
        assert_eq!(listed[0].availability, CredentialAvailability::Missing);
        Ok(())
    }

    #[test]
    fn raw_sqlite_bytes_never_contain_the_entered_secret() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let secret = b"super-secret-value-should-never-be-on-disk";

        {
            let fake = std::sync::Arc::new(FakeSecretStore::new());
            let mut store =
                CredentialStore::open_with_store(&path, Box::new(FakeStoreHandle(fake)))?;
            store.create_with_prompt(
                create_request("OpenAI", "openai"),
                &FakeSecretPrompt::entering(secret),
            )?;
        }

        let raw_bytes = std::fs::read(&path)?;
        let needle = secret.to_vec();
        assert!(
            !raw_bytes
                .windows(needle.len())
                .any(|window| window == needle.as_slice()),
            "the raw SQLite file must never contain the entered secret bytes"
        );
        Ok(())
    }

    #[test]
    fn migrates_v5_settings_schema_to_credentials_schema() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
                idle_unload_minutes INTEGER NOT NULL CHECK (
                    idle_unload_minutes >= 1 AND idle_unload_minutes <= 120
                )
            );
            INSERT INTO app_settings (id, revision, appearance, idle_unload_minutes)
            VALUES (1, 1, 'system', 5);
            CREATE TABLE model_load_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                ownership_state TEXT NOT NULL,
                owned_identifier TEXT,
                owned_model_key TEXT,
                owned_since_unix_seconds INTEGER
            );
            INSERT INTO model_load_state (id, revision, ownership_state) VALUES (1, 1, 'unknown');
            CREATE TABLE conversations (
                id TEXT PRIMARY KEY,
                title TEXT NOT NULL,
                created_at_unix_seconds INTEGER NOT NULL,
                updated_at_unix_seconds INTEGER NOT NULL
            );
            CREATE TABLE messages (
                id TEXT PRIMARY KEY,
                conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
                sequence INTEGER NOT NULL,
                role TEXT NOT NULL,
                text TEXT NOT NULL,
                status TEXT NOT NULL,
                provider_key TEXT,
                model_key TEXT,
                error_message TEXT,
                created_at_unix_seconds INTEGER NOT NULL,
                updated_at_unix_seconds INTEGER NOT NULL,
                UNIQUE (conversation_id, sequence)
            );
            PRAGMA user_version = 5;",
        )?;
        drop(conn);

        let fake = std::sync::Arc::new(FakeSecretStore::new());
        let mut store = CredentialStore::open_with_store(&path, Box::new(FakeStoreHandle(fake)))?;
        store.create_with_prompt(
            create_request("OpenAI", "openai"),
            &FakeSecretPrompt::entering(b"sk-test"),
        )?;
        assert_eq!(store.list()?.len(), 1);
        assert!(super::super::test_support::backup_count(directory.path())? >= 1);
        Ok(())
    }
}

/// Genuine (not faked) evidence against the real macOS Keychain, confirmed
/// feasible in this project's own sandbox (see proposal.md) — unlike
/// llmster's GUI-wake gap, ordinary Keychain Services calls need no
/// interactive session. Only the *storage* side is real here; the native
/// entry *prompt* is still faked (`FakeSecretPrompt`) since driving the
/// real `osascript` dialog is what actually hangs in this environment.
/// Every entry created here is disposable (a fresh UUID account per test)
/// and is always cleaned up via `RealKeychainCleanup`'s `Drop` impl, even
/// if an assertion above it fails.
#[cfg(all(test, target_os = "macos"))]
mod real_keychain_tests {
    use super::*;
    use crate::credentials::{new_secret_store, FakeSecretPrompt};
    use std::error::Error;

    struct RealKeychainCleanup {
        id: String,
    }

    impl Drop for RealKeychainCleanup {
        fn drop(&mut self) {
            let _ = new_secret_store().delete(&self.id);
        }
    }

    #[test]
    fn round_trips_and_deletes_a_real_keychain_entry() -> Result<(), Box<dyn Error>> {
        let mut store = CredentialStore::open_in_memory_with_store(new_secret_store())?;
        let created = store.create_with_prompt(
            CreateCredentialRequest {
                label: "Real keychain test".to_string(),
                provider_key: "test-provider".to_string(),
            },
            &FakeSecretPrompt::entering(b"real-secret-value"),
        )?;
        let _cleanup = RealKeychainCleanup {
            id: created.id.clone(),
        };

        assert_eq!(created.availability, CredentialAvailability::Available);
        assert_eq!(store.resolve_secret(&created.id)?, b"real-secret-value");

        let listed = store.list()?;
        let found = listed
            .iter()
            .find(|reference| reference.id == created.id)
            .ok_or("expected to find the created credential in a real keychain listing")?;
        assert_eq!(found.availability, CredentialAvailability::Available);

        store.delete(DeleteCredentialRequest {
            id: created.id.clone(),
        })?;
        assert!(store.resolve_secret(&created.id).is_err());
        Ok(())
    }

    #[test]
    fn replacing_updates_the_real_keychain_entry() -> Result<(), Box<dyn Error>> {
        let mut store = CredentialStore::open_in_memory_with_store(new_secret_store())?;
        let created = store.create_with_prompt(
            CreateCredentialRequest {
                label: "Real keychain replace test".to_string(),
                provider_key: "test-provider".to_string(),
            },
            &FakeSecretPrompt::entering(b"first-value"),
        )?;
        let _cleanup = RealKeychainCleanup {
            id: created.id.clone(),
        };

        store.replace_with_prompt(
            ReplaceCredentialRequest {
                id: created.id.clone(),
            },
            &FakeSecretPrompt::entering(b"second-value"),
        )?;

        assert_eq!(store.resolve_secret(&created.id)?, b"second-value");
        store.delete(DeleteCredentialRequest {
            id: created.id.clone(),
        })?;
        Ok(())
    }
}
