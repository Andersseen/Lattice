//! The 0.11 remote provider profiles repository: a fourth sibling store
//! type to [`super::SettingsStore`]/[`super::ConversationStore`]/
//! [`super::CredentialStore`], per `storage/mod.rs`'s own doc comment,
//! opening its own `rusqlite::Connection` to the same `lattice.sqlite3`
//! file. Schema evolution lives in `migrations::create_v7_schema`.
//!
//! Two destination-binding invariants are enforced here, structurally
//! rather than by caller discipline:
//!
//! - An update that changes the endpoint clears the credential binding and
//!   the consent in the same statement that changes the endpoint.
//! - `ProviderProfile.consent` is derived as present only while the stored
//!   `consent_endpoint` equals the current `endpoint`, so a stale consent
//!   can never be reported or honored.
//!
//! Nothing in this module performs network I/O: creating, updating,
//! binding, consenting to or listing a profile never contacts its endpoint.

use super::{database, migrations};
use crate::conversations::unix_timestamp_now;
use crate::providers::{
    new_provider_profile_id, validate_credential_id, validate_endpoint, validate_model_key,
    validate_profile_label, BindProviderCredentialRequest, CreateProviderProfileRequest,
    DeleteProviderProfileRequest, GrantProviderConsentRequest, ProviderConsent, ProviderProfile,
    RevokeProviderConsentRequest, UpdateProviderProfileRequest, MAX_PROVIDER_PROFILES,
};
use crate::AppError;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

const PROFILE_COLUMNS: &str = "id, revision, label, endpoint, model_key, credential_id,
    consent_endpoint, consent_granted_at_unix_seconds,
    created_at_unix_seconds, updated_at_unix_seconds";

pub struct ProviderProfileStore {
    conn: Connection,
}

impl ProviderProfileStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref();
        let mut conn = database::open_connection(path)?;
        migrations::migrate(&mut conn, Some(path))?;
        Ok(Self { conn })
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self, AppError> {
        let mut conn = Connection::open_in_memory().map_err(|_| {
            AppError::storage_unavailable("Lattice could not open local provider storage.")
        })?;
        database::apply_connection_pragmas(&conn)?;
        migrations::migrate(&mut conn, None)?;
        Ok(Self { conn })
    }

    /// Lists every profile ordered by label (case-insensitive), then id.
    /// Bounded by `MAX_PROVIDER_PROFILES`, which `create` enforces.
    pub fn list(&self) -> Result<Vec<ProviderProfile>, AppError> {
        let mut statement = self
            .conn
            .prepare(&format!(
                "SELECT {PROFILE_COLUMNS} FROM provider_profiles
                 ORDER BY label COLLATE NOCASE, id
                 LIMIT {MAX_PROVIDER_PROFILES}"
            ))
            .map_err(|_| list_failed())?;
        let rows = statement
            .query_map([], raw_profile_row)
            .map_err(|_| list_failed())?;

        let mut profiles = Vec::new();
        for row in rows {
            profiles.push(row.map_err(|_| list_failed())?.into_profile()?);
        }
        Ok(profiles)
    }

    pub fn get(&self, id: &str) -> Result<ProviderProfile, AppError> {
        self.read_row(id)?.into_profile()
    }

    pub fn create(
        &mut self,
        request: CreateProviderProfileRequest,
    ) -> Result<ProviderProfile, AppError> {
        let label = validate_profile_label(&request.label)?;
        let endpoint = validate_endpoint(&request.endpoint)?;
        let model_key = validate_model_key(&request.model_key)?;
        let credential_id = validate_credential_id(request.credential_id.as_deref())?;

        let count: i64 = self
            .conn
            .query_row("SELECT COUNT(*) FROM provider_profiles", [], |row| {
                row.get(0)
            })
            .map_err(|_| save_failed())?;
        if count >= MAX_PROVIDER_PROFILES {
            return Err(AppError::provider_invalid(
                "Lattice supports up to 50 remote providers.",
            ));
        }

        let id = new_provider_profile_id();
        let now = i64_or_zero(unix_timestamp_now());
        self.conn
            .execute(
                "INSERT INTO provider_profiles (
                    id, revision, label, endpoint, model_key, credential_id,
                    consent_endpoint, consent_granted_at_unix_seconds,
                    created_at_unix_seconds, updated_at_unix_seconds
                 ) VALUES (?1, 1, ?2, ?3, ?4, ?5, NULL, NULL, ?6, ?6)",
                params![id, label, endpoint, model_key, credential_id, now],
            )
            .map_err(map_write_error)?;

        self.get(&id)
    }

    /// Changing the normalized endpoint clears the credential binding and
    /// the consent atomically with the endpoint change; a label- or
    /// model-only update keeps both (providers spec: "Bind Credentials And
    /// Consent To One Destination").
    pub fn update(
        &mut self,
        request: UpdateProviderProfileRequest,
    ) -> Result<ProviderProfile, AppError> {
        let label = validate_profile_label(&request.label)?;
        let endpoint = validate_endpoint(&request.endpoint)?;
        let model_key = validate_model_key(&request.model_key)?;
        let existing = self.read_revision_checked(&request.id, request.expected_revision)?;
        let now = i64_or_zero(unix_timestamp_now());

        let sql = if endpoint == existing.endpoint {
            "UPDATE provider_profiles
             SET label = ?1, endpoint = ?2, model_key = ?3,
                 revision = revision + 1, updated_at_unix_seconds = ?4
             WHERE id = ?5 AND revision = ?6"
        } else {
            "UPDATE provider_profiles
             SET label = ?1, endpoint = ?2, model_key = ?3,
                 credential_id = NULL, consent_endpoint = NULL,
                 consent_granted_at_unix_seconds = NULL,
                 revision = revision + 1, updated_at_unix_seconds = ?4
             WHERE id = ?5 AND revision = ?6"
        };
        let changed = self
            .conn
            .execute(
                sql,
                params![
                    label,
                    endpoint,
                    model_key,
                    now,
                    request.id,
                    existing.revision
                ],
            )
            .map_err(map_write_error)?;
        ensure_changed(changed)?;

        self.get(&request.id)
    }

    pub fn delete(&mut self, request: DeleteProviderProfileRequest) -> Result<(), AppError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM provider_profiles WHERE id = ?1",
                params![request.id],
            )
            .map_err(|_| AppError::storage_unavailable("Lattice could not delete the provider."))?;
        if changed == 0 {
            return Err(not_found());
        }
        Ok(())
    }

    /// Binds (or, with `None`, unbinds) a credential reference for the
    /// profile's current endpoint. An unknown credential is refused by the
    /// `credentials(id)` foreign key, never stored.
    pub fn bind_credential(
        &mut self,
        request: BindProviderCredentialRequest,
    ) -> Result<ProviderProfile, AppError> {
        let credential_id = validate_credential_id(request.credential_id.as_deref())?;
        let existing = self.read_revision_checked(&request.id, request.expected_revision)?;
        let now = i64_or_zero(unix_timestamp_now());

        let changed = self
            .conn
            .execute(
                "UPDATE provider_profiles
                 SET credential_id = ?1, revision = revision + 1, updated_at_unix_seconds = ?2
                 WHERE id = ?3 AND revision = ?4",
                params![credential_id, now, request.id, existing.revision],
            )
            .map_err(map_write_error)?;
        ensure_changed(changed)?;

        self.get(&request.id)
    }

    /// Records consent for exactly the endpoint the disclosure named, and
    /// only while it is still the profile's endpoint.
    pub fn grant_consent(
        &mut self,
        request: GrantProviderConsentRequest,
    ) -> Result<ProviderProfile, AppError> {
        let existing = self.read_revision_checked(&request.id, request.expected_revision)?;
        let shown_endpoint =
            validate_endpoint(&request.endpoint).map_err(|_| endpoint_changed())?;
        if shown_endpoint != existing.endpoint {
            return Err(endpoint_changed());
        }
        let now = i64_or_zero(unix_timestamp_now());

        let changed = self
            .conn
            .execute(
                "UPDATE provider_profiles
                 SET consent_endpoint = ?1, consent_granted_at_unix_seconds = ?2,
                     revision = revision + 1, updated_at_unix_seconds = ?2
                 WHERE id = ?3 AND revision = ?4 AND endpoint = ?1",
                params![shown_endpoint, now, request.id, existing.revision],
            )
            .map_err(map_write_error)?;
        ensure_changed(changed)?;

        self.get(&request.id)
    }

    pub fn revoke_consent(
        &mut self,
        request: RevokeProviderConsentRequest,
    ) -> Result<ProviderProfile, AppError> {
        let existing = self.read_revision_checked(&request.id, request.expected_revision)?;
        let now = i64_or_zero(unix_timestamp_now());

        let changed = self
            .conn
            .execute(
                "UPDATE provider_profiles
                 SET consent_endpoint = NULL, consent_granted_at_unix_seconds = NULL,
                     revision = revision + 1, updated_at_unix_seconds = ?1
                 WHERE id = ?2 AND revision = ?3",
                params![now, request.id, existing.revision],
            )
            .map_err(map_write_error)?;
        ensure_changed(changed)?;

        self.get(&request.id)
    }

    fn read_row(&self, id: &str) -> Result<RawProfileRow, AppError> {
        self.conn
            .query_row(
                &format!("SELECT {PROFILE_COLUMNS} FROM provider_profiles WHERE id = ?1"),
                params![id],
                raw_profile_row,
            )
            .optional()
            .map_err(|_| AppError::storage_unavailable("Lattice could not read the provider."))?
            .ok_or_else(not_found)
    }

    fn read_revision_checked(
        &self,
        id: &str,
        expected_revision: u64,
    ) -> Result<RawProfileRow, AppError> {
        let row = self.read_row(id)?;
        if database::validate_revision(row.revision)? != expected_revision {
            return Err(stale_revision());
        }
        Ok(row)
    }
}

/// Read-validation for `migrations::migrate`'s "schema already current"
/// branch, mirroring every other domain's sanity read.
pub(super) fn read_provider_profiles_sanity(conn: &Connection) -> Result<(), AppError> {
    conn.query_row("SELECT COUNT(*) FROM provider_profiles", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|_| AppError::storage_unavailable("Lattice could not read local providers."))?;
    Ok(())
}

struct RawProfileRow {
    id: String,
    revision: i64,
    label: String,
    endpoint: String,
    model_key: String,
    credential_id: Option<String>,
    consent_endpoint: Option<String>,
    consent_granted_at_unix_seconds: Option<i64>,
    created_at_unix_seconds: i64,
    updated_at_unix_seconds: i64,
}

impl RawProfileRow {
    fn into_profile(self) -> Result<ProviderProfile, AppError> {
        let consent = match (self.consent_endpoint, self.consent_granted_at_unix_seconds) {
            (Some(consent_endpoint), Some(granted_at)) if consent_endpoint == self.endpoint => {
                Some(ProviderConsent {
                    endpoint: consent_endpoint,
                    granted_at_unix_seconds: nonneg_u64(granted_at),
                })
            }
            _ => None,
        };

        Ok(ProviderProfile {
            id: self.id,
            revision: database::validate_revision(self.revision)?,
            label: self.label,
            endpoint: self.endpoint,
            model_key: self.model_key,
            credential_id: self.credential_id,
            consent,
            created_at_unix_seconds: nonneg_u64(self.created_at_unix_seconds),
            updated_at_unix_seconds: nonneg_u64(self.updated_at_unix_seconds),
        })
    }
}

fn raw_profile_row(row: &Row<'_>) -> rusqlite::Result<RawProfileRow> {
    Ok(RawProfileRow {
        id: row.get(0)?,
        revision: row.get(1)?,
        label: row.get(2)?,
        endpoint: row.get(3)?,
        model_key: row.get(4)?,
        credential_id: row.get(5)?,
        consent_endpoint: row.get(6)?,
        consent_granted_at_unix_seconds: row.get(7)?,
        created_at_unix_seconds: row.get(8)?,
        updated_at_unix_seconds: row.get(9)?,
    })
}

fn is_foreign_key_violation(error: &rusqlite::Error) -> bool {
    matches!(
        error,
        rusqlite::Error::SqliteFailure(failure, _)
            if failure.extended_code == rusqlite::ffi::SQLITE_CONSTRAINT_FOREIGNKEY
    )
}

fn map_write_error(error: rusqlite::Error) -> AppError {
    if is_foreign_key_violation(&error) {
        AppError::provider_invalid("That credential no longer exists.")
    } else {
        save_failed()
    }
}

fn ensure_changed(changed: usize) -> Result<(), AppError> {
    if changed == 0 {
        return Err(stale_revision());
    }
    Ok(())
}

fn not_found() -> AppError {
    AppError::provider_not_found("That provider no longer exists.")
}

fn stale_revision() -> AppError {
    AppError::provider_conflict("This provider changed; reload it and try again.")
}

fn endpoint_changed() -> AppError {
    AppError::provider_conflict("The provider's endpoint changed; review it before approving.")
}

fn list_failed() -> AppError {
    AppError::storage_unavailable("Lattice could not list providers.")
}

fn save_failed() -> AppError {
    AppError::storage_unavailable("Lattice could not save the provider.")
}

fn nonneg_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn i64_or_zero(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::super::credentials::CredentialStore;
    use super::*;
    use crate::credentials::{
        CreateCredentialRequest, DeleteCredentialRequest, FakeSecretPrompt, FakeSecretStore,
    };
    use crate::providers::{prepare_remote_target, ChatMessage, ChatRequest, ChatRole};
    use std::{error::Error, net::TcpListener};
    use tempfile::tempdir;

    fn insert_credential(conn: &Connection, id: &str) -> Result<(), Box<dyn Error>> {
        conn.execute(
            "INSERT INTO credentials (id, label, provider_key, created_at_unix_seconds, updated_at_unix_seconds)
             VALUES (?1, 'Key', 'openai', 1, 1)",
            params![id],
        )?;
        Ok(())
    }

    fn create_request(credential_id: Option<&str>) -> CreateProviderProfileRequest {
        CreateProviderProfileRequest {
            label: "Example".to_string(),
            endpoint: "https://api.example.com/v1/".to_string(),
            model_key: "example-model".to_string(),
            credential_id: credential_id.map(str::to_string),
        }
    }

    fn consented(
        store: &mut ProviderProfileStore,
        credential_id: Option<&str>,
    ) -> Result<ProviderProfile, Box<dyn Error>> {
        let created = store.create(create_request(credential_id))?;
        Ok(store.grant_consent(GrantProviderConsentRequest {
            id: created.id,
            expected_revision: created.revision,
            endpoint: created.endpoint,
        })?)
    }

    #[test]
    fn creates_lists_and_gets_a_normalized_profile() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        insert_credential(&store.conn, "credential-1")?;

        let created = store.create(create_request(Some("credential-1")))?;

        assert_eq!(created.revision, 1);
        assert_eq!(created.endpoint, "https://api.example.com/v1");
        assert_eq!(created.credential_id.as_deref(), Some("credential-1"));
        assert_eq!(created.consent, None);
        assert_eq!(store.list()?, vec![created.clone()]);
        assert_eq!(store.get(&created.id)?, created);
        Ok(())
    }

    #[test]
    fn lists_profiles_by_label_case_insensitively() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        for label in ["beta", "Alpha", "gamma"] {
            store.create(CreateProviderProfileRequest {
                label: label.to_string(),
                ..create_request(None)
            })?;
        }

        let labels: Vec<String> = store
            .list()?
            .into_iter()
            .map(|profile| profile.label)
            .collect();
        assert_eq!(labels, ["Alpha", "beta", "gamma"]);
        Ok(())
    }

    #[test]
    fn invalid_input_is_refused_and_nothing_is_stored() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        for request in [
            CreateProviderProfileRequest {
                endpoint: "http://api.example.com/v1".to_string(),
                ..create_request(None)
            },
            CreateProviderProfileRequest {
                label: " ".to_string(),
                ..create_request(None)
            },
            CreateProviderProfileRequest {
                model_key: String::new(),
                ..create_request(None)
            },
        ] {
            let error = store.create(request).err().ok_or("expected refusal")?;
            assert_eq!(error.code, "provider.invalid");
        }
        assert!(store.list()?.is_empty());
        Ok(())
    }

    #[test]
    fn binding_an_unknown_credential_is_refused() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        let error = store
            .create(create_request(Some("missing")))
            .err()
            .ok_or("expected refusal")?;
        assert_eq!(error.code, "provider.invalid");
        assert!(store.list()?.is_empty());

        let created = store.create(create_request(None))?;
        let error = store
            .bind_credential(BindProviderCredentialRequest {
                id: created.id.clone(),
                expected_revision: created.revision,
                credential_id: Some("missing".to_string()),
            })
            .err()
            .ok_or("expected refusal")?;
        assert_eq!(error.code, "provider.invalid");
        assert_eq!(store.get(&created.id)?, created);
        Ok(())
    }

    #[test]
    fn binds_and_unbinds_a_credential() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        insert_credential(&store.conn, "credential-1")?;
        let created = store.create(create_request(None))?;

        let bound = store.bind_credential(BindProviderCredentialRequest {
            id: created.id.clone(),
            expected_revision: created.revision,
            credential_id: Some("credential-1".to_string()),
        })?;
        assert_eq!(bound.credential_id.as_deref(), Some("credential-1"));
        assert_eq!(bound.revision, 2);

        let unbound = store.bind_credential(BindProviderCredentialRequest {
            id: created.id,
            expected_revision: bound.revision,
            credential_id: None,
        })?;
        assert_eq!(unbound.credential_id, None);
        Ok(())
    }

    #[test]
    fn deleting_a_credential_through_its_store_unbinds_every_profile() -> Result<(), Box<dyn Error>>
    {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let mut profiles = ProviderProfileStore::open(&path)?;
        insert_credential(&profiles.conn, "credential-1")?;
        let first = profiles.create(create_request(Some("credential-1")))?;
        let second = profiles.create(create_request(Some("credential-1")))?;

        let mut credentials =
            CredentialStore::open_with_store(&path, Box::new(FakeSecretStore::new()))?;
        credentials.delete(DeleteCredentialRequest {
            id: "credential-1".to_string(),
        })?;

        assert_eq!(profiles.get(&first.id)?.credential_id, None);
        assert_eq!(profiles.get(&second.id)?.credential_id, None);
        Ok(())
    }

    #[test]
    fn a_stale_revision_modifies_nothing() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        let created = consented(&mut store, None)?;
        let stale = created.revision - 1;

        let attempts: Vec<Result<ProviderProfile, AppError>> = vec![
            store.update(UpdateProviderProfileRequest {
                id: created.id.clone(),
                expected_revision: stale,
                label: "Renamed".to_string(),
                endpoint: created.endpoint.clone(),
                model_key: created.model_key.clone(),
            }),
            store.bind_credential(BindProviderCredentialRequest {
                id: created.id.clone(),
                expected_revision: stale,
                credential_id: None,
            }),
            store.grant_consent(GrantProviderConsentRequest {
                id: created.id.clone(),
                expected_revision: stale,
                endpoint: created.endpoint.clone(),
            }),
            store.revoke_consent(RevokeProviderConsentRequest {
                id: created.id.clone(),
                expected_revision: stale,
            }),
        ];
        for attempt in attempts {
            assert_eq!(
                attempt.err().ok_or("expected conflict")?.code,
                "provider.conflict"
            );
        }
        assert_eq!(store.get(&created.id)?, created);
        Ok(())
    }

    #[test]
    fn changing_the_endpoint_clears_credential_binding_and_consent() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        insert_credential(&store.conn, "credential-1")?;
        let created = consented(&mut store, Some("credential-1"))?;
        assert!(created.consent.is_some());

        let moved = store.update(UpdateProviderProfileRequest {
            id: created.id.clone(),
            expected_revision: created.revision,
            label: created.label.clone(),
            endpoint: "https://other.example.com/v1".to_string(),
            model_key: created.model_key.clone(),
        })?;

        assert_eq!(moved.endpoint, "https://other.example.com/v1");
        assert_eq!(moved.credential_id, None);
        assert_eq!(moved.consent, None);
        Ok(())
    }

    #[test]
    fn changing_only_label_or_model_keeps_destination_bindings() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        insert_credential(&store.conn, "credential-1")?;
        let created = consented(&mut store, Some("credential-1"))?;

        let renamed = store.update(UpdateProviderProfileRequest {
            id: created.id.clone(),
            expected_revision: created.revision,
            label: "Renamed".to_string(),
            // A different spelling of the same normalized destination.
            endpoint: "HTTPS://API.EXAMPLE.COM/v1/".to_string(),
            model_key: "other-model".to_string(),
        })?;

        assert_eq!(renamed.label, "Renamed");
        assert_eq!(renamed.model_key, "other-model");
        assert_eq!(renamed.credential_id.as_deref(), Some("credential-1"));
        assert_eq!(
            renamed
                .consent
                .as_ref()
                .map(|consent| consent.endpoint.as_str()),
            Some("https://api.example.com/v1")
        );
        Ok(())
    }

    #[test]
    fn consent_is_granted_only_for_the_endpoint_shown() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        let created = store.create(create_request(None))?;

        let error = store
            .grant_consent(GrantProviderConsentRequest {
                id: created.id.clone(),
                expected_revision: created.revision,
                endpoint: "https://attacker.example.net/v1".to_string(),
            })
            .err()
            .ok_or("expected refusal")?;
        assert_eq!(error.code, "provider.conflict");
        assert_eq!(store.get(&created.id)?.consent, None);

        let granted = store.grant_consent(GrantProviderConsentRequest {
            id: created.id.clone(),
            expected_revision: created.revision,
            endpoint: created.endpoint.clone(),
        })?;
        assert_eq!(
            granted.consent.map(|consent| consent.endpoint),
            Some(created.endpoint)
        );
        Ok(())
    }

    #[test]
    fn consent_can_be_revoked() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        let created = consented(&mut store, None)?;

        let revoked = store.revoke_consent(RevokeProviderConsentRequest {
            id: created.id,
            expected_revision: created.revision,
        })?;
        assert_eq!(revoked.consent, None);
        Ok(())
    }

    #[test]
    fn a_consent_recorded_for_another_endpoint_is_never_reported() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        let created = consented(&mut store, None)?;
        store.conn.execute(
            "UPDATE provider_profiles SET consent_endpoint = 'https://stale.example.com/v1' WHERE id = ?1",
            params![created.id],
        )?;

        assert_eq!(store.get(&created.id)?.consent, None);
        Ok(())
    }

    #[test]
    fn deletes_a_profile_and_reports_unknown_ids() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        let created = store.create(create_request(None))?;

        store.delete(DeleteProviderProfileRequest {
            id: created.id.clone(),
        })?;
        assert!(store.list()?.is_empty());

        for error in [
            store
                .delete(DeleteProviderProfileRequest {
                    id: created.id.clone(),
                })
                .err(),
            store.get(&created.id).err(),
        ] {
            assert_eq!(
                error.ok_or("expected not found")?.code,
                "provider.not_found"
            );
        }
        Ok(())
    }

    #[test]
    fn the_number_of_profiles_is_bounded() -> Result<(), Box<dyn Error>> {
        let mut store = ProviderProfileStore::open_in_memory()?;
        for _ in 0..MAX_PROVIDER_PROFILES {
            store.create(create_request(None))?;
        }
        let error = store
            .create(create_request(None))
            .err()
            .ok_or("expected the cap to refuse")?;
        assert_eq!(error.code, "provider.invalid");
        Ok(())
    }

    /// Providers spec: "Profile Management And Selection Send No Network
    /// Traffic". The profile points at a real listener; after every profile
    /// operation, that listener has still received no connection.
    #[test]
    fn profile_management_never_contacts_the_endpoint() -> Result<(), Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        listener.set_nonblocking(true)?;
        let endpoint = format!("https://127.0.0.1:{}/v1", listener.local_addr()?.port());

        let mut store = ProviderProfileStore::open_in_memory()?;
        insert_credential(&store.conn, "credential-1")?;
        let created = store.create(CreateProviderProfileRequest {
            endpoint: endpoint.clone(),
            ..create_request(Some("credential-1"))
        })?;
        let updated = store.update(UpdateProviderProfileRequest {
            id: created.id.clone(),
            expected_revision: created.revision,
            label: "Renamed".to_string(),
            endpoint: endpoint.clone(),
            model_key: "m".to_string(),
        })?;
        let bound = store.bind_credential(BindProviderCredentialRequest {
            id: created.id.clone(),
            expected_revision: updated.revision,
            credential_id: Some("credential-1".to_string()),
        })?;
        let granted = store.grant_consent(GrantProviderConsentRequest {
            id: created.id.clone(),
            expected_revision: bound.revision,
            endpoint,
        })?;
        store.list()?;
        store.get(&created.id)?;
        store.revoke_consent(RevokeProviderConsentRequest {
            id: created.id.clone(),
            expected_revision: granted.revision,
        })?;
        store.delete(DeleteProviderProfileRequest { id: created.id })?;

        std::thread::sleep(std::time::Duration::from_millis(100));
        assert!(
            listener.accept().is_err(),
            "no profile operation may open a connection"
        );
        Ok(())
    }

    /// Providers spec: "A Profile References A Credential Without Containing
    /// Its Secret". A real credential is created, bound and resolved into a
    /// remote target through the same path `start_chat_stream` uses; the
    /// raw database file must still never contain the secret bytes.
    #[test]
    fn raw_sqlite_bytes_never_contain_a_bound_and_resolved_secret() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let secret = b"sk-profile-secret-that-must-never-reach-sqlite";

        {
            let mut credentials =
                CredentialStore::open_with_store(&path, Box::new(FakeSecretStore::new()))?;
            let credential = credentials.create_with_prompt(
                CreateCredentialRequest {
                    label: "Remote key".to_string(),
                    provider_key: "openai".to_string(),
                },
                &FakeSecretPrompt::entering(secret),
            )?;

            let mut profiles = ProviderProfileStore::open(&path)?;
            let profile = consented(&mut profiles, Some(&credential.id))?;
            let request = ChatRequest {
                model_key: profile.model_key.clone(),
                messages: vec![ChatMessage {
                    role: ChatRole::User,
                    text: "hi".to_string(),
                }],
            };
            let target =
                prepare_remote_target(&profile, &request, |id| credentials.resolve_secret(id))?;
            assert!(!format!("{target:?}").contains("sk-profile-secret"));
        }

        let raw_bytes = std::fs::read(&path)?;
        assert!(
            !raw_bytes
                .windows(secret.len())
                .any(|window| window == secret.as_slice()),
            "the raw SQLite file must never contain a bound credential's secret"
        );
        Ok(())
    }

    #[test]
    fn migrates_v6_schema_to_provider_profiles_keeping_existing_data() -> Result<(), Box<dyn Error>>
    {
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
            INSERT INTO conversations VALUES ('conversation-1', 'Kept', 1, 1);
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
            CREATE TABLE credentials (
                id TEXT PRIMARY KEY,
                label TEXT NOT NULL,
                provider_key TEXT NOT NULL,
                created_at_unix_seconds INTEGER NOT NULL,
                updated_at_unix_seconds INTEGER NOT NULL
            );
            INSERT INTO credentials VALUES ('credential-1', 'Kept key', 'openai', 1, 1);
            PRAGMA user_version = 6;",
        )?;
        drop(conn);

        let mut store = ProviderProfileStore::open(&path)?;
        let created = store.create(create_request(Some("credential-1")))?;
        assert_eq!(created.credential_id.as_deref(), Some("credential-1"));

        let kept: i64 = store.conn.query_row(
            "SELECT COUNT(*) FROM conversations WHERE id = 'conversation-1'",
            [],
            |row| row.get(0),
        )?;
        assert_eq!(kept, 1);
        let version: i64 = store
            .conn
            .pragma_query_value(None, "user_version", |row| row.get(0))?;
        assert_eq!(version, 7);
        assert!(super::super::test_support::backup_count(directory.path())? >= 1);
        Ok(())
    }
}
