//! Fail-closed stubs for every non-macOS target: no plaintext fallback
//! exists to fall back to, by construction. Every operation refuses
//! immediately and every credential reports `Unsupported`. Not exercised
//! by this workspace's own CI (macOS-only today per the Definition of v1),
//! but kept buildable so a future Linux/Windows preview build compiles.

use super::{PromptOutcome, SecretPrompt, SecretStore, SecretStoreError};
use crate::AppError;

pub(crate) fn new_secret_store() -> Box<dyn SecretStore> {
    Box::new(UnsupportedSecretStore)
}

pub(crate) fn new_secret_prompt() -> Box<dyn SecretPrompt> {
    Box::new(UnsupportedSecretPrompt)
}

struct UnsupportedSecretStore;

impl SecretStore for UnsupportedSecretStore {
    fn set(&self, _account: &str, _secret: &[u8]) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::Unsupported)
    }

    fn get(&self, _account: &str) -> Result<Vec<u8>, SecretStoreError> {
        Err(SecretStoreError::Unsupported)
    }

    fn delete(&self, _account: &str) -> Result<(), SecretStoreError> {
        Err(SecretStoreError::Unsupported)
    }
}

struct UnsupportedSecretPrompt;

impl SecretPrompt for UnsupportedSecretPrompt {
    fn prompt(&self) -> Result<PromptOutcome, AppError> {
        Err(AppError::credential_unsupported(
            "Secure credential entry is not supported on this platform.",
        ))
    }
}

/// Compiled and run on every platform (including macOS, where this is not
/// the active implementation) so the fail-closed contract itself always has
/// direct test coverage, not just on a target this workspace has no CI for.
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_secret_store_operation_reports_unsupported() {
        let store = new_secret_store();
        assert_eq!(
            store.set("account", b"secret"),
            Err(SecretStoreError::Unsupported)
        );
        assert_eq!(store.get("account"), Err(SecretStoreError::Unsupported));
        assert_eq!(store.delete("account"), Err(SecretStoreError::Unsupported));
    }

    #[test]
    fn the_prompt_refuses_with_a_safe_unsupported_error() {
        let prompt = new_secret_prompt();
        let error = match prompt.prompt() {
            Err(error) => error,
            Ok(_) => unreachable!("UnsupportedSecretPrompt never succeeds"),
        };
        assert_eq!(error.code, "credential.unsupported");
    }
}
