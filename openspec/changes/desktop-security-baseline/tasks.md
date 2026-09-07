# Tasks

- [x] Verify 0.2 is archived: permanent specs include `application-api`, active `application-runtime-foundation` is reconciled, and its required checks are recorded.
- [x] Implement production CSP in `apps/desktop/src-tauri/tauri.conf.json` and add config tests for denied production allowances.
- [x] Restrict custom application command exposure in `apps/desktop/src-tauri/build.rs` using the 0.2 command inventory.
- [x] Minimize `apps/desktop/src-tauri/capabilities/default.json` permissions to current main-window foundation needs.
- [ ] Add automated or recorded native denial checks for unauthorized window/origin command invocation.
- [ ] Preserve Home/System rendering and native metadata IPC under production CSP.
- [ ] Add WebView evidence for blocked remote script and unapproved connect attempts.
- [x] Make fatal desktop startup failure exit unsuccessfully and cover the exit policy with Rust tests or a deterministic harness.
- [x] Add scheduled and manifest/lockfile-triggered JavaScript/Rust dependency advisory checks with documented severity/exception policy.
- [ ] Capture P0 release idle resource evidence: five-minute settled idle, three launch/exit cycles, memory, CPU, child processes, hardware/OS/build metadata, and uncertainty.
- [ ] Update permanent `desktop-security` spec, ADR, security policy, verification record, capability map, and changelog only to match verified behavior.
- [ ] Run required checks for the candidate commit: OpenSpec validation, `pnpm check`, `pnpm build:web`, relevant Playwright/native smoke, `cargo fmt --all --check`, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`, and `cargo test --workspace`.
