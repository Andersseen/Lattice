# 0013 - Native macOS Credential Storage

## Status

Accepted

## Context

0.10 must let Rust provision, list, replace, and revoke named credential references without ever exposing the secret value to Angular ("no value in DOM, IPC responses, SQLite, logs or argv" — `docs/roadmap.md`), ahead of 0.11+'s remote providers. Two separate problems need solving: where the secret is _stored_, and how it is _entered_ without the value ever touching the WebView.

## Decision

**Storage:** `security-framework` `3.7.0` (`default-features = false`) in `crates/lattice-core`, checked against docs.rs/crates.io on 2026-09-13. Confirmed by running `cargo add security-framework@3.7.0 --no-default-features` against this workspace (then reverted, exploratory) that it resolves exactly two new packages, `security-framework` and `security-framework-sys` — `core-foundation`/`core-foundation-sys` were already present transitively through Tauri's own macOS window-management stack, and building `lattice-desktop` afterward showed `security-framework` itself is _also_ already pulled in transitively by `tao`/`tauri-runtime-wry`, making this an even smaller net addition than initially estimated. Confirmed exact signatures directly against `3.7.0`'s own docs (not an older version): `set_generic_password_options(password, options)`, `generic_password(options)`, `delete_generic_password(service, account)`, and `security_framework::base::Error::code() -> OSStatus`/`.message()`. `PasswordOptions::set_access_synchronized(Some(false))` is set explicitly on every write so credentials never enter iCloud Keychain sync — this is local-first application data, not meant to follow the user to another device.

Rejected: shelling out to `/usr/bin/security add-generic-password -w <secret>` (the secret would sit in that child process's argv, exactly what the roadmap forbids); the `keyring` crate (on macOS it is itself a thin wrapper over `security-framework`, adding a layer with no benefit while this scope is macOS-only).

**Native secure entry:** `osascript`/AppleScript's `display dialog ... with hidden answer` (zero new dependency; already present on every macOS system), invoked with a **fixed, never-interpolated** prompt string — identifying which credential is being entered is the Angular-side UI's job, shown before the dialog opens, specifically so a user-typed label or provider key can never become part of the AppleScript source and create an injection surface. Bounded by AppleScript's own `giving up after 120` clause _and_ an independent ~125-second Rust-side hard timeout that kills the child process if it hangs.

Rejected: a Tauri native dialog or second WebView window with a password `<input>` (still a WebView under the hood — the secret would still transiently exist in a DOM/JS runtime, which is exactly what "no value in DOM" rules out); a compiled native Cocoa helper via raw `objc`/`cocoa` FFI (real added build/unsafe-surface complexity for exactly what AppleScript's own dialog already provides as a genuinely native, masked text field).

**Empirical findings from this session, not asserted from documentation alone:** the `security` CLI (add/find/delete-generic-password against a disposable test entry) completed in under a second in this project's own sandbox, with no GUI prompt — confirming real macOS Keychain access is genuinely testable in automated Rust tests here, unlike llmster's GUI-wake gap (ADR 0010's context). Conversely, `osascript -e 'display dialog ... with hidden answer'` hung indefinitely and had to be killed manually, with no sign its own `giving up after` clause ever engaged — this sandbox has no interactive WindowServer/Aqua session, the same class of gap already recorded for `open -a "LM Studio"` (0.6–0.8's verification docs). This is why the Rust-side timeout is independent rather than relying on AppleScript's clause alone, and why the real dialog invocation itself is recorded as pending manual/native evidence rather than covered by an automated test.

`crates/lattice-core/src/credentials/` isolates every OS-specific call behind two small traits, `SecretStore` (keychain read/write/delete) and `SecretPrompt` (native entry) — `macos.rs` (`#[cfg(target_os = "macos")]`) is the only file allowed to call `security_framework` or spawn `osascript`; `unsupported.rs` (every other target) fails closed with no plaintext fallback. `storage/credentials.rs`'s `CredentialStore` is a third sibling store type alongside `SettingsStore`/`ConversationStore` (ADR 0009, extended by 0.9's `ConversationStore` precedent) — only reference metadata (label, provider key, timestamps) lives in `lattice.sqlite3`; the secret itself never does, structurally (the `credentials` table has no secret column).

Full behavioral detail lives in `openspec/changes/os-secure-credentials/design.md`; this ADR records the dependency/mechanism decisions, not the implementation.

## Consequences

- The workspace gains a macOS-only credential storage/entry mechanism confined to `lattice-core::credentials`; no plaintext fallback exists on any platform, by construction (the `unsupported` stub cannot be bypassed to write a secret in the clear).
- Real macOS Keychain round-trips are covered by genuine (not faked) automated Rust tests, a stronger evidence bar than most prior native-boundary work in this project could achieve — the native entry dialog itself remains manual/pending evidence for the same environment reason ADR 0010/0012 already record.
- Credentials are recorded per the roadmap's `CredentialRef` contract with no secret field anywhere in its wire shape, locked down by a dedicated wire-shape test; 0.11+ resolves a credential's secret only through a Rust-internal `resolve_secret` call, never through IPC.
- Signing/entitlement behavior for a distributed, notarized build (keychain-access-groups, Hardened Runtime) is out of scope for 0.10 and belongs to 0.27's packaging work, not assumed correct here.
