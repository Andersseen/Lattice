# Desktop Security Baseline Design

## Current State

- `apps/desktop/src-tauri/tauri.conf.json` sets `app.security.csp` to `null`.
- `apps/desktop/src-tauri/capabilities/default.json` grants `core:default` and `log:default` to window label `main`.
- `apps/desktop/src-tauri/build.rs` calls `tauri_build::build()` without an app manifest command inventory, so custom command restriction is not explicit.
- `apps/desktop/src-tauri/src/lib.rs` prints fatal shell startup errors but does not guarantee an unsuccessful process exit.
- No current code exposes process, filesystem, terminal, credential, provider, storage, MCP, or remote UI behavior.

## Tauri Security Inputs

Tauri v2 documentation consulted on 2026-09-07:

- Production CSP is enabled only when configured and is injected for bundled non-dev asset responses.
- CSP should be as restrictive as possible and allow only trusted application resources.
- Required IPC commonly uses `ipc:` and `http://ipc.localhost` in `connect-src`.
- Tauri appends generated nonces and hashes for bundled application assets at compile time.
- Custom application commands are available unless explicitly restricted through `tauri_build::AppManifest::commands(...)`.
- Capability JSON files scope permissions to window labels and platforms.

## Production CSP

Set a non-null production CSP in `tauri.conf.json` that allows bundled app resources and required IPC only:

- `default-src`: `'self' customprotocol: asset:`
- `script-src`: `'self'`
- `style-src`: `'self'` plus the narrowest style exception proven necessary for Angular/Tauri generated styles; prefer nonce/hash support where compatible.
- `img-src`: `'self' data: asset: http://asset.localhost`
- `font-src`: `'self' data:`
- `connect-src`: `ipc: http://ipc.localhost`
- `object-src`: `'none'`
- `base-uri`: `'none'`
- `frame-src`: `'none'`

`unsafe-eval`, wildcard remote origins, public HTTP(S) script hosts, and production dev-server origins are not allowed. If Angular inline styles require an exception, document the exact reason and prove blocked script/connect behavior still holds.

Development keeps `devUrl` and HMR behavior in the existing Tauri dev flow. Do not broaden production CSP to make dev mode convenient.

## Command And Window Authority

Use the generated command inventory from 0.2 as the single source for custom app command names.

Update `apps/desktop/src-tauri/build.rs` to use `tauri_build::try_build(...)` with `AppManifest::commands(&[GET_APP_INFO_COMMAND])`, or a generated equivalent that does not duplicate the command string by hand. If build script access to `lattice-core` is required, admit it as a build dependency in `lattice-desktop` because it is already the authoritative current consumer.

Replace broad capability grants with the minimum permissions required by the current shell:

- Main window label only: `main`.
- Current custom command: metadata read only.
- Core permissions only when the UI actively needs them for the foundation shell.
- Log permissions only if frontend log plugin use is demonstrated by current code; otherwise remove log WebView authority while keeping Rust-side logging.

Denial evidence must exercise an unauthorized label/origin/window, not merely inspect JSON.

## Startup Failure

Split desktop boot into a fallible function that returns an error to `main`. The binary main should print a safe startup message and exit with a non-zero status on fatal shell startup failure. Product internals remain in logs/stderr, not in Angular state.

Add a deterministic failure harness where possible. If Tauri runtime failure is not reliably inducible in unit tests, cover the pure exit-policy function with Rust tests and record manual native failure evidence.

## Dependency And Advisory Policy

Add scheduled and change-triggered dependency checks without adding product runtime dependencies:

- JavaScript: `pnpm audit` with a documented blocking threshold.
- Rust: `cargo audit --deny yanked` or an equivalent maintained RustSec advisory check, installed in CI/tooling only.
- Unmaintained or unsound warnings are triaged and tracked; they do not fail ordinary PRs unless the advisory is explicitly escalated by policy.
- License/maintenance exceptions are documented with owner, reason, severity, expiry/revisit date, and mitigation.
- Manifest/lockfile changes require advisory review before merge.

Do not make external-service advisory outages fail every ordinary PR forever. Scheduled failures or missing release evidence block 0.3 completion until triaged.

## P0 Resource Evidence

Record P0 under `docs/verification/` using the Definition of v1 method:

- Release build.
- No runtime/model/MCP/provider/workspace/task capability active.
- Five-minute settled idle sample.
- Three launch/exit cycles.
- Native/WebView aggregate physical memory, CPU, child process list, exact hardware, OS, app commit, command versions, and uncertainty.

No new resource-monitoring daemon is introduced in 0.3. Use OS tools and recorded commands/manual evidence. Unexpected children, non-idle CPU above the v1 gate, or unbounded resource growth block completion.

## Verification Plan

Cheap PR checks:

- Config tests assert CSP is non-null and lacks `unsafe-eval`, wildcard remote script/connect origins, and production dev-server allowances.
- Capability/build tests assert custom commands are explicitly registered and only the main window has current grants.
- Existing 0.2 IPC/contract tests continue to pass.
- Startup exit-policy Rust tests cover fatal failure behavior.
- Dependency advisory workflow/config is syntax checked.

Native/release evidence:

- Production WebView renders Home/System and metadata IPC succeeds.
- Remote script and unapproved connect attempts are blocked and recorded.
- Unauthorized window/origin command invocation is denied and recorded.
- P0 resource baseline is captured.

## Documentation

Implementation updates:

- Permanent `desktop-security` spec after verification.
- A new ADR for the security baseline and any style/CSP exception.
- `SECURITY.md` or security policy documentation for dependency/advisory expectations.
- `docs/verification/0.3-desktop-security.md` with command output summaries and native/manual evidence.

Do not update capability status to implemented until native and P0 evidence exist.
