# Changelog

All notable changes to Lattice will be documented here.

The project follows SemVer once releases begin. Lattice is currently pre-1.0 and has no published releases.

## Unreleased

- Initial open source foundation.
- Added Tauri 2 desktop shell.
- Added Angular zoneless frontend.
- Added minimal Rust core and typed IPC smoke path.
- Added Rust-owned generated application IPC contracts, runtime decoding, safe bridge error normalization, and latest-refresh ordering.
- Added Bun, Turborepo, Cargo workspaces, tests, CI, docs, ADRs, and OpenSpec setup.
- Replaced Prettier and generic TypeScript ESLint with Biome, keeping Angular ESLint for Angular-specific component and template validation.
- Added provider-independent conversation persistence: a sibling SQLite `ConversationStore`, create/list/reopen/delete with pagination, bounded checkpointing of streaming replies, restart-to-interrupted reconciliation, and a History page alongside Chat.
- Added OS-secure credential references: a sibling SQLite `CredentialStore` storing only label/provider metadata, a real macOS Keychain-backed secret store with native (non-WebView) secure entry, and list/create/replace/delete controls in Settings — no command ever returns a secret value.
- Added remote OpenAI-compatible chat: HTTPS provider profiles whose credential binding and consent are tied to one endpoint, a per-request local/remote Chat provider selector with a disclosure before the first send to an endpoint, per-reply provider/model provenance, and one OpenAI-compatible adapter for local and remote destinations (TLS via rustls, no redirects or environment proxies, normalized auth/rate-limit/timeout errors, best-effort connection close on cancel).
