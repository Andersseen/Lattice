//! The only file allowed to call into `security_framework` or spawn
//! `osascript`. `MacosKeychainSecretStore` is the real [`super::SecretStore`]
//! backed by Keychain Services; `OsascriptSecretPrompt` is the real
//! [`super::SecretPrompt`], a genuinely native (Cocoa alert panel, not a
//! WebView) masked text prompt via AppleScript's `display dialog ... with
//! hidden answer`. See design.md's "Dependency admission" for why these two
//! mechanisms were chosen over a Tauri dialog, `keyring`, or raw Cocoa FFI.

use super::{PromptOutcome, SecretPrompt, SecretStore, SecretStoreError, KEYCHAIN_SERVICE};
use crate::AppError;
use security_framework::base::Error as SecurityError;
use security_framework::passwords::{
    delete_generic_password, generic_password, set_generic_password_options, PasswordOptions,
};
use std::{
    io::Read,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

const ERR_SEC_ITEM_NOT_FOUND: i32 = -25300;
const ERR_SEC_INTERACTION_NOT_ALLOWED: i32 = -25308;
const ERR_SEC_AUTH_FAILED: i32 = -25293;

pub(crate) fn new_secret_store() -> Box<dyn SecretStore> {
    Box::new(MacosKeychainSecretStore)
}

pub(crate) fn new_secret_prompt() -> Box<dyn SecretPrompt> {
    Box::new(OsascriptSecretPrompt)
}

struct MacosKeychainSecretStore;

impl SecretStore for MacosKeychainSecretStore {
    fn set(&self, account: &str, secret: &[u8]) -> Result<(), SecretStoreError> {
        let mut options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, account);
        // Keeps the credential in the local keychain only — this is
        // local-first application data, never meant to sync via iCloud
        // Keychain to other devices.
        options.set_access_synchronized(Some(false));
        set_generic_password_options(secret, options).map_err(classify_error)
    }

    fn get(&self, account: &str) -> Result<Vec<u8>, SecretStoreError> {
        let options = PasswordOptions::new_generic_password(KEYCHAIN_SERVICE, account);
        generic_password(options).map_err(classify_error)
    }

    fn delete(&self, account: &str) -> Result<(), SecretStoreError> {
        match delete_generic_password(KEYCHAIN_SERVICE, account) {
            Ok(()) => Ok(()),
            Err(error) if error.code() == ERR_SEC_ITEM_NOT_FOUND => Ok(()),
            Err(error) => Err(classify_error(error)),
        }
    }
}

fn classify_error(error: SecurityError) -> SecretStoreError {
    match error.code() {
        ERR_SEC_ITEM_NOT_FOUND => SecretStoreError::NotFound,
        ERR_SEC_INTERACTION_NOT_ALLOWED | ERR_SEC_AUTH_FAILED => SecretStoreError::Locked,
        _ => SecretStoreError::Failed,
    }
}

/// Fixed, never-interpolated prompt text — see design.md: identifying which
/// credential is being entered is the Angular-side UI's job, shown before
/// this dialog opens, precisely so a label or provider key a user typed can
/// never become part of the AppleScript source. `giving up after 120` is
/// AppleScript's own dismissal clause; `PROMPT_TIMEOUT` below is a second,
/// independent Rust-side bound confirmed necessary in this project's own
/// sandbox testing (see proposal.md) — a missing WindowServer/Aqua session
/// hung `display dialog` well past any sign of the AppleScript clause
/// engaging, so the Rust-side timeout is what actually prevents a hang.
const PROMPT_SCRIPT: &str = "display dialog \"Enter the credential value.\" default answer \"\" \
     with hidden answer with title \"Lattice\" giving up after 120";
const PROMPT_TIMEOUT: Duration = Duration::from_secs(125);
const PROMPT_POLL_INTERVAL: Duration = Duration::from_millis(200);

struct OsascriptSecretPrompt;

impl SecretPrompt for OsascriptSecretPrompt {
    fn prompt(&self) -> Result<PromptOutcome, AppError> {
        let mut child = Command::new("osascript")
            .arg("-e")
            .arg(PROMPT_SCRIPT)
            .arg("-e")
            .arg("text returned of result")
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|_| {
                AppError::credential_unavailable("Lattice could not open the secure entry prompt.")
            })?;

        // Pipe buffers comfortably hold a short entered secret (a few
        // hundred bytes at most), so reading them only after the process
        // exits, rather than concurrently while polling, is a deliberate
        // simplification for this narrow, small-output use case — not the
        // general-purpose streaming pattern `providers::completion` needs.
        let mut stdout_handle = child.stdout.take();
        let mut stderr_handle = child.stderr.take();

        let deadline = Instant::now() + PROMPT_TIMEOUT;
        let exited = loop {
            match child.try_wait() {
                Ok(Some(status)) => break Some(status),
                Ok(None) => {
                    if Instant::now() >= deadline {
                        break None;
                    }
                    thread::sleep(PROMPT_POLL_INTERVAL);
                }
                Err(_) => {
                    return Err(AppError::credential_unavailable(
                        "Lattice could not read the secure entry prompt.",
                    ));
                }
            }
        };

        let Some(status) = exited else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(AppError::credential_unavailable(
                "The secure entry prompt did not respond in time.",
            ));
        };

        let mut stdout_bytes = Vec::new();
        if let Some(mut stdout) = stdout_handle.take() {
            let _ = stdout.read_to_end(&mut stdout_bytes);
        }
        let mut stderr_bytes = Vec::new();
        if let Some(mut stderr) = stderr_handle.take() {
            let _ = stderr.read_to_end(&mut stderr_bytes);
        }

        interpret_prompt_output(status.success(), &stdout_bytes, &stderr_bytes)
    }
}

fn interpret_prompt_output(
    success: bool,
    stdout: &[u8],
    stderr: &[u8],
) -> Result<PromptOutcome, AppError> {
    if success {
        let mut text = String::from_utf8_lossy(stdout).into_owned();
        while text.ends_with('\n') || text.ends_with('\r') {
            text.pop();
        }
        return Ok(PromptOutcome::Entered(text.into_bytes()));
    }

    let stderr_text = String::from_utf8_lossy(stderr).to_lowercase();
    if stderr_text.contains("-128") || stderr_text.contains("user canceled") {
        return Ok(PromptOutcome::Cancelled);
    }

    Err(AppError::credential_unavailable(
        "Lattice could not read the secure entry prompt.",
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn interprets_a_successful_entry() -> Result<(), Box<dyn Error>> {
        let outcome = interpret_prompt_output(true, b"my-secret\n", b"")?;
        match outcome {
            PromptOutcome::Entered(secret) => assert_eq!(secret, b"my-secret"),
            PromptOutcome::Cancelled => return Err("expected Entered".into()),
        }
        Ok(())
    }

    #[test]
    fn interprets_user_cancellation() -> Result<(), Box<dyn Error>> {
        let outcome =
            interpret_prompt_output(false, b"", b"execution error: User canceled. (-128)")?;
        assert!(matches!(outcome, PromptOutcome::Cancelled));
        Ok(())
    }

    #[test]
    fn interprets_an_unexpected_failure() -> Result<(), Box<dyn Error>> {
        let error = interpret_prompt_output(false, b"", b"execution error: something else")
            .err()
            .ok_or("expected a failure")?;
        assert_eq!(error.code, "credential.unavailable");
        Ok(())
    }
}
