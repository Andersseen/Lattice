# Desktop Security Baseline

## Objective

Restrict the shipped desktop WebView and native command exposure before Lattice adds process, storage, provider, filesystem, terminal, MCP, or credential capabilities.

## Rationale

The current foundation intentionally exposes only application metadata, but the desktop shell still ships with `csp: null`, broad default core/log grants, custom command exposure that is not explicitly permissioned, and startup failure handling that can print an error without a guaranteed unsuccessful exit. Later roadmap minors add native capabilities with larger blast radius, so 0.3 establishes the restrictive baseline first.

Tauri v2 documentation consulted on 2026-09-07 confirms that production CSP is injected for bundled assets and that restricting custom application commands requires explicit app-manifest command registration during build.

## Dependencies

- 0.2 Application Runtime Foundation implemented, verified, reconciled into permanent specs, and archived.
- Current foundation metadata command remains the only application command in scope.

## Scope

- Configure a non-null production CSP for bundled Angular assets and required Tauri IPC.
- Keep development allowances separate from production security policy.
- Restrict application command and window authority to the trusted main window.
- Replace broad default desktop grants with narrowly justified core/log permissions.
- Ensure fatal desktop startup failure is visible and exits unsuccessfully.
- Add reproducible tests/manual evidence for production native rendering, working IPC, blocked script/connect/resource loads, denied unauthorized windows/origins, startup failure behavior, and P0 idle resource capture.
- Introduce scheduled dependency/advisory checks and a documented policy for lockfile/manifest changes.

## Non-goals

- No generalized authorization engine.
- No credentials, settings, SQLite, model runtime, provider, filesystem, terminal, MCP, memory, Space, task, scheduling, or new product routes.
- No remote executable UI, remote WebView IPC authority, or blanket CSP wildcard.
- No UI redesign beyond changes required to keep existing Home/System rendering under CSP.

## Impacted Capabilities

- `foundation`
- `desktop-security`
