# OS-Secure Credentials Design

## Ownership

New domain area, sibling to `conversations` (0.9), not an extension of `SettingsStore`/`ConversationStore`: `credentials` owns credential reference identity, availability status, and the two small trait boundaries (`SecretStore`, `SecretPrompt`) that isolate the only two places OS-specific/native code is allowed to run. `lattice-core` owns every canonical type, the storage repository, and both trait definitions; `lattice-desktop` only wires IPC commands and picks the real trait implementations at startup. Angular owns only the Credentials section of the Settings page and never sees a secret value in any form.

New modules:

- `crates/lattice-core/src/credentials/mod.rs` — canonical `CredentialRef`, `CredentialAvailability`, ID generation, label/provider-key validation, and the `SecretStore`/`SecretPrompt` trait definitions plus their `Fake*` test implementations.
- `crates/lattice-core/src/credentials/macos.rs` (`#[cfg(target_os = "macos")]`) — `MacosKeychainSecretStore` (via `security-framework`) and `OsascriptSecretPrompt`. This is the only file allowed to call into `security_framework` or spawn `osascript`.
- `crates/lattice-core/src/credentials/unsupported.rs` (`#[cfg(not(target_os = "macos"))]`) — `UnsupportedSecretStore`/`UnsupportedSecretPrompt`, both fail-closed.
- `crates/lattice-core/src/storage/credentials.rs` — `CredentialStore`, the sibling repository, sharing `lattice.sqlite3` with `SettingsStore`/`ConversationStore` exactly as `ConversationStore` already does (third independent `rusqlite::Connection`, same `busy_timeout`/`foreign_keys` pragmas, same `migrations::migrate()` cascade).

## Schema (version 6)

```sql
CREATE TABLE credentials (
  id TEXT PRIMARY KEY,
  label TEXT NOT NULL,
  provider_key TEXT NOT NULL,
  created_at_unix_seconds INTEGER NOT NULL,
  updated_at_unix_seconds INTEGER NOT NULL
);
CREATE INDEX idx_credentials_updated ON credentials(updated_at_unix_seconds DESC, id DESC);
```

No secret column exists, structurally — the table cannot leak a secret it never has a place to hold. `id` is an app-generated UUID v4 (never a provider-issued value), reused directly as the keychain **account** name; the keychain **service** name is the single fixed constant `LATTICE_KEYCHAIN_SERVICE = "dev.lattice.credentials"`, so every credential lives in one namespaced service regardless of `provider_key`, and `provider_key` is purely descriptive metadata (no closed enum: 0.11–0.13 haven't defined a final provider-key vocabulary yet, so this avoids a premature/speculative registry, consistent with `architecture-sequence.md`'s "IDs are added with consumers, not a universal object system").

## Contracts

```rust
// crates/lattice-core/src/credentials/mod.rs
pub enum CredentialAvailability { Available, Locked, Missing, Unsupported }

pub struct CredentialRef {
    pub id: String,
    pub label: String,
    pub provider_key: String,
    pub availability: CredentialAvailability,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
    // No secret field. Never will be. Enforced by a wire-shape test asserting
    // the serialized keys of `CredentialRef` never include anything secret-shaped.
}
```

IPC (Tauri command / `wire.rs` names):

```rust
pub const LIST_CREDENTIALS_COMMAND: &str = "list_credentials";
pub const CREATE_CREDENTIAL_COMMAND: &str = "create_credential";
pub const REPLACE_CREDENTIAL_COMMAND: &str = "replace_credential";
pub const DELETE_CREDENTIAL_COMMAND: &str = "delete_credential";

pub struct CreateCredentialRequest { pub label: String, pub provider_key: String }
pub struct ReplaceCredentialRequest { pub id: String }
pub struct DeleteCredentialRequest { pub id: String }
```

`create_credential`/`replace_credential` return `CredentialRef` on success; a user-cancelled native entry returns `AppError::credential_cancelled` (recoverable, not a failure the UI should treat as an error banner). There is intentionally no request type carrying a secret — Angular has no code path that could construct one.

## Secret storage and native entry: two small trait boundaries, not one

```rust
pub trait SecretStore: Send + Sync {
    fn set(&self, account: &str, secret: &[u8]) -> Result<(), AppError>;
    fn get(&self, account: &str) -> Result<Vec<u8>, AppError>;
    fn delete(&self, account: &str) -> Result<(), AppError>;
    fn lock_status(&self) -> SecretStoreStatus; // Unlocked | Locked | Unsupported
}

pub trait SecretPrompt: Send + Sync {
    fn prompt(&self) -> Result<PromptOutcome, AppError>; // Entered(Vec<u8>) | Cancelled
}
```

Kept as two separate traits, not one combined "credential backend" trait: `SecretStore` is pure OS-keychain I/O (used by `list`/`create`/`replace`/`delete`/`resolve_secret`), while `SecretPrompt` is pure UI acquisition (used only by `create`/`replace`, and only to obtain the bytes before handing them to `SecretStore::set`). This mirrors 0.9's own separation of concerns (`ConversationStore` vs. the checkpoint policy) and lets tests fake exactly one axis at a time — e.g. a test proving "cancellation writes nothing" only needs a `FakeSecretPrompt`, not a fake keychain at all.

**`MacosKeychainSecretStore`** (`credentials/macos.rs`, `#[cfg(target_os = "macos")]`): wraps `security_framework::passwords::{set_generic_password, generic_password, delete_generic_password}` against the fixed service constant. `lock_status()` inspects `security_framework::os::macos::keychain::SecKeychain::default()`'s status flags for `LOCKED` without attempting a read (never triggers a surprise unlock prompt outside an explicit user-initiated flow); a `get`/`set` that still fails with `errSecInteractionNotAllowed` (-25308) after that check reports `Locked` defensively. `errSecItemNotFound` (-25300) from `get`/`delete` is treated as "absent," not an error, wherever the caller only needs to know presence (e.g. `list`'s `Missing` derivation).

**`OsascriptSecretPrompt`** (`credentials/macos.rs`): spawns

```
osascript -e 'display dialog "Enter the credential value." default answer "" with hidden answer with title "Lattice" giving up after 120' -e 'text returned of result'
```

via `std::process::Command`, fixed argv, no shell. The prompt text is a **hard-coded literal, never interpolated with `label`/`provider_key`** — this removes any AppleScript-injection surface a malicious or malformed label/provider-key string could otherwise create; identifying _which_ credential is being entered is the Angular-side UI's job (shown before the native dialog opens), not the native dialog's own text. Reads the child's stdout on a dedicated thread; the calling thread waits up to **125 seconds** (5s over AppleScript's own 120s `giving up after`) before killing the child and returning `AppError::credential_unavailable` — defense-in-depth confirmed necessary in this session: a genuinely absent WindowServer/Aqua session hung `display dialog` past 8 seconds with no sign of AppleScript's own timeout ever engaging, so the Rust-side bound is what actually prevents an indefinite hang, not the AppleScript clause alone. Exit status containing `-128` ("User canceled") maps to `PromptOutcome::Cancelled`, never an error.

**`UnsupportedSecretStore`/`UnsupportedSecretPrompt`** (`credentials/unsupported.rs`, every other target): `lock_status()` always `Unsupported`; every `set`/`get`/`delete`/`prompt` call refuses immediately with `AppError::credential_unsupported` — no plaintext fallback exists to fall back to, by construction.

**`FakeSecretStore`/`FakeSecretPrompt`** (`credentials/mod.rs`, `#[cfg(test)]`): in-memory `Mutex<HashMap<String, Vec<u8>>>` and a configurable fixed outcome, used by every `storage::credentials` unit test.

## `CredentialStore` behavior

- `list() -> Vec<CredentialRef>`: reads all metadata rows, calls `secret_store.lock_status()` **once** (not per row — one keychain-level status applies to every entry), and for each row derives `Available` (unlocked, entry present), `Locked` (keychain locked), `Missing` (unlocked but `get` reports absent — e.g. deleted outside Lattice), or `Unsupported`.
- `create(label, provider_key, prompt) -> Result<CredentialRef, AppError>`: validates `label`/`provider_key` (non-empty, bounded length), runs `prompt.prompt()`; on `Cancelled` returns `AppError::credential_cancelled` touching neither store; on `Entered(secret)`, writes the keychain entry **first**, then the SQLite row only if that succeeds (if the SQLite insert then fails, the keychain entry is deleted again — no orphaned secret survives a failed reference).
- `replace(id, prompt) -> Result<CredentialRef, AppError>`: same prompt flow against an existing row; refuses with `AppError::credential_not_found` if the id is unknown; only bumps `updated_at_unix_seconds` on success.
- `delete(id) -> Result<(), AppError>`: deletes the SQLite row, then best-effort deletes the keychain entry (a `Missing`/already-absent keychain entry is not an error at this point — the reference is gone either way).
- `resolve_secret(id) -> Result<Vec<u8>, AppError>`: Rust-internal only, deliberately **not** wired to any Tauri command — exists now because the roadmap's 0.10 exit criterion requires "Rust can resolve an authorized reference," and 0.11+ will call it directly from `lattice-core`, not through IPC.

## Security invariants (each backed by a test)

- `CredentialRef`'s serialized JSON shape never contains a secret-shaped field — locked down by a wire-shape test mirroring `chat_stream_event_serializes_with_camel_case_kind_tag`'s precedent.
- Creating a credential and then reading the raw `lattice.sqlite3` file's bytes directly (bypassing the repository) never contains the entered secret text.
- Cancelling a native entry (`FakeSecretPrompt` configured to `Cancelled`) leaves both the SQLite table and the fake keychain untouched.
- A `FakeSecretStore` configured to fail `set` leaves no SQLite row behind (`create`'s rollback path).
- No command handler, log line, or error message ever includes secret bytes — verified by code inspection (no `AppError`/log call in `credentials`/`storage::credentials`/the Tauri command handlers ever formats a secret value) plus the JSON/DB tests above.

## Dependency admission

**Keychain storage.** Considered:

- **Shell out to `/usr/bin/security add-generic-password -w <secret>`.** Rejected outright: the roadmap explicitly forbids the secret in argv, and `-w` takes the password as a literal command-line argument — visible (briefly) to any other process on the same machine via `ps`, and a real, not hypothetical, concern this crate exists specifically to avoid.
- **`keyring` crate (cross-platform keychain wrapper).** Rejected for this scope: on macOS it is itself a thin layer over `security-framework`/Keychain Services, adding an abstraction with no benefit while 0.10 is macOS-only (Windows/Linux report `unsupported`, not "attempt cross-platform storage"); revisit only if Windows/Linux real storage is ever scoped.
- **Raw `objc`/`cocoa` FFI to Keychain Services.** Rejected: strictly more code and unsafe surface than `security-framework` already provides as a safe, maintained wrapper over the identical C API.
- **`security-framework` `3.7.0`, `default-features = false`.** Accepted. MIT OR Apache-2.0, MSRV 1.85 (workspace already requires a newer stable toolchain). Confirmed 2026-09-13: `cargo add security-framework@3.7.0 --no-default-features` followed by `cargo build -p lattice-core` compiled cleanly; confirmed directly in the resulting `Cargo.lock` (then reverted, this being exploratory) that exactly two new packages resolve — `security-framework` and `security-framework-sys` — since `core-foundation`/`core-foundation-sys` are already present transitively through Tauri's macOS toolchain. Exact function signatures (`set_generic_password`, `generic_password`, `delete_generic_password`, `Error::code()`/`message()`) confirmed against docs.rs for `3.7.0` directly, not assumed from an older version's docs.

**Native secure entry.** Considered:

- **A Tauri native dialog / second WebView window with a password `<input>`.** Rejected: still a WebView under the hood (WKWebView on macOS) — the secret would still transiently exist in a DOM/JS runtime, which is exactly what "no value in DOM" (roadmap) rules out, regardless of which window hosts it.
- **A compiled native Cocoa helper (`NSSecureTextField` via a small Swift/Obj-C binary or `objc`/`cocoa` crates).** Rejected: real added build complexity (a second toolchain or a nontrivial unsafe FFI surface) for exactly what AppleScript's `display dialog ... with hidden answer` already provides as a masked, genuinely-native (Cocoa alert panel, not a WebView) text field, with zero new dependencies.
- **`osascript`/AppleScript `display dialog`.** Accepted. Zero new dependency (already present on every macOS system); the one identified risk (AppleScript injection via interpolated text) is eliminated by never interpolating caller-provided text into the script — the prompt text is a fixed literal. The one identified reliability risk (hanging with no interactive session, confirmed in this sandbox) is mitigated by a Rust-side hard timeout independent of AppleScript's own `giving up after`.

## Verification

Testing tier follows 0.9's persistence-and-native-boundary precedent: `FakeSecretStore`/`FakeSecretPrompt`-backed unit tests for all `CredentialStore` logic (hermetic, run everywhere), plus a genuinely new category this change can afford that 0.6–0.9 could not — a small `#[cfg(target_os = "macos")]` test module round-tripping `MacosKeychainSecretStore` against real, disposable, UUID-suffixed keychain entries with guaranteed cleanup, confirmed feasible directly in this sandbox (see proposal.md). The real `OsascriptSecretPrompt` invocation is not exercised automatically (confirmed to hang in this sandbox); it is recorded as pending native/manual evidence in `docs/verification/0.10-os-secure-credentials.md`, the same honest-limitation pattern already used for llmster's GUI-wake gap.
