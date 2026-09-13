//! Credential reference identity, availability status, and the two small
//! trait boundaries that isolate every OS-specific/native call this domain
//! makes: [`SecretStore`] (keychain read/write/delete/lock-status) and
//! [`SecretPrompt`] (native secure text entry). Persistence against these
//! types lives in `storage::credentials`, mirroring `conversations`'s own
//! domain/storage split from 0.9. The real macOS implementations of both
//! traits live in `macos` (`#[cfg(target_os = "macos")]`); every other
//! target uses `unsupported`'s fail-closed stubs. Neither trait, nor
//! [`CredentialRef`], nor any request/response type in this module ever
//! carries a secret value — see the credentials spec's "Lattice SHALL
//! Acquire A New Or Replacement Secret Through Native Entry Only".

#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(target_os = "macos"))]
mod unsupported;

#[cfg(target_os = "macos")]
pub(crate) use macos::{new_secret_prompt, new_secret_store};
#[cfg(not(target_os = "macos"))]
pub(crate) use unsupported::{new_secret_prompt, new_secret_store};

use serde::{Deserialize, Serialize};

pub const LIST_CREDENTIALS_COMMAND: &str = "list_credentials";
pub const CREATE_CREDENTIAL_COMMAND: &str = "create_credential";
pub const REPLACE_CREDENTIAL_COMMAND: &str = "replace_credential";
pub const DELETE_CREDENTIAL_COMMAND: &str = "delete_credential";

/// The single fixed keychain "service" namespace every credential lives
/// under, regardless of `provider_key`. `provider_key` is descriptive
/// metadata only (no closed enum yet — 0.11-0.13 haven't defined a final
/// provider-key vocabulary), never part of keychain identity.
pub(crate) const KEYCHAIN_SERVICE: &str = "dev.lattice.credentials";

pub(crate) const MAX_LABEL_CHARS: usize = 120;
pub(crate) const MAX_PROVIDER_KEY_CHARS: usize = 60;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum CredentialAvailability {
    Available,
    Locked,
    Missing,
    Unsupported,
}

/// A credential reference: label, provider key, availability and
/// timestamps. Deliberately has no secret field and never will — Angular
/// only ever sees this shape, never a resolved value.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CredentialRef {
    pub id: String,
    pub label: String,
    pub provider_key: String,
    pub availability: CredentialAvailability,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateCredentialRequest {
    pub label: String,
    pub provider_key: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplaceCredentialRequest {
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteCredentialRequest {
    pub id: String,
}

/// The result of one native secure-entry attempt.
#[derive(Debug)]
pub enum PromptOutcome {
    Entered(Vec<u8>),
    Cancelled,
}

/// Why a `SecretStore` operation didn't return a value, distinguished
/// precisely enough for `storage::credentials::CredentialStore::list` to
/// derive an honest [`CredentialAvailability`] per row: `NotFound` means
/// the keychain itself is reachable but this particular entry is absent
/// (deleted outside Lattice) — never confused with `Locked`/`Failed`,
/// which mean the keychain could not be consulted at all right now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecretStoreError {
    NotFound,
    Locked,
    // Only ever constructed by `unsupported::UnsupportedSecretStore`, which
    // is compiled solely for non-macOS targets — genuinely unreachable on
    // this workspace's only currently built/tested platform (macOS), not
    // an oversight. `storage::credentials` still matches this arm so the
    // mapping stays correct once a non-macOS build exists.
    #[allow(dead_code)]
    Unsupported,
    Failed,
}

impl std::fmt::Display for SecretStoreError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Self::NotFound => "not found",
            Self::Locked => "locked",
            Self::Unsupported => "unsupported",
            Self::Failed => "failed",
        };
        write!(formatter, "secret store error: {label}")
    }
}

impl std::error::Error for SecretStoreError {}

/// OS keychain read/write/delete. Implemented by
/// `macos::MacosKeychainSecretStore` (real) and
/// `unsupported::UnsupportedSecretStore` (fail-closed); tests use
/// [`FakeSecretStore`]. `Send + Sync` so `Box<dyn SecretStore>` can live
/// inside `storage::credentials::CredentialStore` behind a `Mutex`. Returns
/// [`SecretStoreError`] rather than `AppError` directly: the same
/// underlying failure means different things to different callers (`list`
/// turns it into an availability status; `create`/`replace`/`delete` turn
/// it into a terminal `AppError`).
pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, secret: &[u8]) -> Result<(), SecretStoreError>;
    fn get(&self, account: &str) -> Result<Vec<u8>, SecretStoreError>;
    fn delete(&self, account: &str) -> Result<(), SecretStoreError>;
}

/// Native secure text entry. Implemented by `macos::OsascriptSecretPrompt`
/// (real) and `unsupported::UnsupportedSecretPrompt`; tests use
/// [`FakeSecretPrompt`].
pub trait SecretPrompt: Send + Sync {
    fn prompt(&self) -> Result<PromptOutcome, crate::AppError>;
}

pub fn new_credential_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn validate_label(label: &str) -> Result<String, crate::AppError> {
    let trimmed = label.trim();
    if trimmed.is_empty() {
        return Err(crate::AppError::credential_not_found(
            "Give the credential a label.",
        ));
    }
    if trimmed.chars().count() > MAX_LABEL_CHARS {
        return Err(crate::AppError::credential_not_found(
            "The credential label is too long.",
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn validate_provider_key(provider_key: &str) -> Result<String, crate::AppError> {
    let trimmed = provider_key.trim();
    if trimmed.is_empty() {
        return Err(crate::AppError::credential_not_found(
            "Give the credential a provider key.",
        ));
    }
    if trimmed.chars().count() > MAX_PROVIDER_KEY_CHARS {
        return Err(crate::AppError::credential_not_found(
            "The credential provider key is too long.",
        ));
    }
    Ok(trimmed.to_string())
}

pub(crate) fn unix_timestamp_now() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

#[cfg(test)]
pub(crate) struct FakeSecretStore {
    entries: std::sync::Mutex<std::collections::HashMap<String, Vec<u8>>>,
    fail_next_set: std::sync::atomic::AtomicBool,
    locked: std::sync::atomic::AtomicBool,
}

#[cfg(test)]
impl FakeSecretStore {
    pub(crate) fn new() -> Self {
        Self {
            entries: std::sync::Mutex::new(std::collections::HashMap::new()),
            fail_next_set: std::sync::atomic::AtomicBool::new(false),
            locked: std::sync::atomic::AtomicBool::new(false),
        }
    }

    pub(crate) fn set_locked(&self, locked: bool) {
        self.locked
            .store(locked, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn fail_next_set(&self) {
        self.fail_next_set
            .store(true, std::sync::atomic::Ordering::SeqCst);
    }

    pub(crate) fn contains(&self, account: &str) -> bool {
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .contains_key(account)
    }
}

#[cfg(test)]
impl SecretStore for FakeSecretStore {
    fn set(&self, account: &str, secret: &[u8]) -> Result<(), SecretStoreError> {
        if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SecretStoreError::Locked);
        }
        if self
            .fail_next_set
            .swap(false, std::sync::atomic::Ordering::SeqCst)
        {
            return Err(SecretStoreError::Failed);
        }
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .insert(account.to_string(), secret.to_vec());
        Ok(())
    }

    fn get(&self, account: &str) -> Result<Vec<u8>, SecretStoreError> {
        if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SecretStoreError::Locked);
        }
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .get(account)
            .cloned()
            .ok_or(SecretStoreError::NotFound)
    }

    fn delete(&self, account: &str) -> Result<(), SecretStoreError> {
        if self.locked.load(std::sync::atomic::Ordering::SeqCst) {
            return Err(SecretStoreError::Locked);
        }
        self.entries
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .remove(account);
        Ok(())
    }
}

#[cfg(test)]
pub(crate) struct FakeSecretPrompt {
    outcome: std::sync::Mutex<Option<Vec<u8>>>,
}

#[cfg(test)]
impl FakeSecretPrompt {
    pub(crate) fn entering(secret: &[u8]) -> Self {
        Self {
            outcome: std::sync::Mutex::new(Some(secret.to_vec())),
        }
    }

    pub(crate) fn cancelling() -> Self {
        Self {
            outcome: std::sync::Mutex::new(None),
        }
    }
}

#[cfg(test)]
impl SecretPrompt for FakeSecretPrompt {
    fn prompt(&self) -> Result<PromptOutcome, crate::AppError> {
        Ok(
            match self
                .outcome
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner)
                .clone()
            {
                Some(secret) => PromptOutcome::Entered(secret),
                None => PromptOutcome::Cancelled,
            },
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn generates_distinct_uuid_credential_ids() {
        let first = new_credential_id();
        let second = new_credential_id();
        assert_ne!(first, second);
        assert_eq!(first.len(), 36);
    }

    #[test]
    fn trims_and_accepts_a_valid_label() -> Result<(), Box<dyn Error>> {
        assert_eq!(validate_label("  OpenAI  ")?, "OpenAI");
        Ok(())
    }

    #[test]
    fn rejects_blank_label() -> Result<(), Box<dyn Error>> {
        let error = validate_label("   ")
            .err()
            .ok_or("expected a validation error")?;
        assert_eq!(error.code, "credential.not_found");
        Ok(())
    }

    #[test]
    fn rejects_oversized_label() {
        let long_label = "a".repeat(MAX_LABEL_CHARS + 1);
        assert!(validate_label(&long_label).is_err());
    }

    #[test]
    fn rejects_blank_provider_key() {
        assert!(validate_provider_key("").is_err());
    }

    #[test]
    fn fake_secret_store_round_trips_and_reports_missing_after_delete() -> Result<(), Box<dyn Error>>
    {
        let store = FakeSecretStore::new();
        store.set("acct-1", b"shh")?;
        assert_eq!(store.get("acct-1")?, b"shh");

        store.delete("acct-1")?;
        assert!(store.get("acct-1").is_err());
        Ok(())
    }

    #[test]
    fn fake_secret_prompt_reports_configured_outcome() -> Result<(), Box<dyn Error>> {
        let entering = FakeSecretPrompt::entering(b"value");
        match entering.prompt()? {
            PromptOutcome::Entered(secret) => assert_eq!(secret, b"value"),
            PromptOutcome::Cancelled => return Err("expected Entered".into()),
        }

        let cancelling = FakeSecretPrompt::cancelling();
        assert!(matches!(cancelling.prompt()?, PromptOutcome::Cancelled));
        Ok(())
    }
}
