# OS-Secure Credentials

## Objective

Let Rust provision, list, replace, and revoke named credential references backed by the OS keychain, with the secret value entered through a genuinely native (non-WebView) prompt and never exposed to Angular, so 0.11+ remote providers have something authorized to attach to.

## Rationale

The critical path (`docs/roadmap.md`, "Critical path and parallel opportunities") requires credentials before any remote provider request: "Persistence precedes history, credentials precede remote requests." 0.9 (`conversation-persistence`) just established the pattern this change reuses — a sibling SQLite domain behind its own store type, sharing one file/migration authority with `SettingsStore`/`ConversationStore` (`crates/lattice-core/src/storage/mod.rs`'s own doc comment; ADR 0009). Credentials extend that pattern one level further: the SQLite row holds only a reference (label, provider key, timestamps) — the secret itself lives exclusively in the OS keychain, never in `lattice.sqlite3`, never in an IPC response, never in argv.

Checked 2026-09-13 against docs.rs and crates.io: `security-framework` `3.7.0` (MIT OR Apache-2.0, MSRV 1.85) provides direct Rust bindings to Apple's Keychain Services (`set_generic_password`/`generic_password`/`delete_generic_password`), avoiding the CLI-argv exposure a `security add-generic-password -w <secret>` shell-out would create — the roadmap explicitly forbids the secret ever appearing in argv. `cargo add security-framework@3.7.0 --no-default-features` was run against this workspace (then reverted) to confirm exactly two new packages resolve (`security-framework`, `security-framework-sys`); `core-foundation`/`core-foundation-sys` already exist transitively via Tauri's macOS stack. See design.md's "Dependency admission" for the full comparison, including why the native secure-entry prompt is `osascript`/AppleScript rather than a new dependency or a Tauri WebView dialog (which would not satisfy "no secret in DOM").

Two real environment findings from this session's exploration, not asserted from documentation alone: the `security` CLI (add/find/delete-generic-password against a disposable test entry) completed in under a second in this very sandbox with no GUI prompt — real macOS Keychain access is genuinely testable here, unlike llmster's GUI-wake gap. Conversely, `osascript -e 'display dialog ... with hidden answer'` hung indefinitely and had to be killed manually — this sandbox has no interactive WindowServer/Aqua session, the same class of gap already recorded for `open -a "LM Studio"` in 0.6–0.8's verification docs. The native-entry dialog is implemented and bounded by its own timeout, but its real interactive behavior is pending manual native evidence, not automated-test evidence.

## Dependencies

- 0.3 desktop security baseline: the deny-by-default command/permission surface this change's new commands register into.
- 0.4 persistent application settings (ADR 0009) and 0.9 conversation persistence: the SQLite storage-module pattern (`storage::mod.rs`'s sibling-module-and-store-type guidance, the versioned `migrate()` cascade, `busy_timeout`/`foreign_keys` pragmas) this change's `CredentialStore` extends to schema version 6.

## Scope

- New `credentials` domain (`crates/lattice-core/src/credentials/`): `CredentialRef` (id, label, provider key, availability status, timestamps — no secret field, ever), ID generation, validation.
- New `storage/credentials.rs` repository (`CredentialStore`, a third sibling store type alongside `SettingsStore`/`ConversationStore`) storing only reference metadata; schema version 6.
- A `SecretStore` trait abstracting the actual keychain read/write/delete/lock-status, with a real macOS implementation (`security-framework`), a fail-closed stub for unsupported platforms, and a fake for tests.
- A `SecretPrompt` trait abstracting native secure text entry, with a real `osascript`/AppleScript implementation (fixed, non-interpolated prompt text; bounded by both AppleScript's own `giving up after` and a Rust-side hard subprocess timeout) and a fake for tests.
- `list_credentials`, `create_credential`, `replace_credential`, `delete_credential` IPC commands — no command ever returns the secret value.
- A "Credentials" section added to the existing Settings page (Angular): list with status badges, add/replace/delete actions, no secret input field anywhere in Angular.

## Non-goals

- Remote provider requests, OAuth flows, or any network egress using a resolved credential (0.11+).
- Custom cryptography or a Lattice-owned secret store; the OS keychain is the only backing store.
- A plaintext fallback on any platform; unsupported platforms fail closed.
- Angular password/secret input forms of any kind.
- Programmatic cancellation of an in-progress native entry (the user cancels through the native dialog itself).
- Real credential storage on Windows/Linux (reported `unsupported`, not attempted).
- Credential rotation history, multiple secrets per reference, or per-provider validation of the entered value.

## Impacted Capabilities

- `credentials` (new)

`local-storage`'s existing generic requirements (schema versioning/migration/backup, Rust-only storage authority) already cover this change's schema-6 migration without new wording, exactly as they did for 0.9. `application-api`'s generic IPC-contract-drift requirement already covers the new commands.
