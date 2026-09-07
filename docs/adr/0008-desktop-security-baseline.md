# ADR 0008: Desktop Security Baseline

- Status: Accepted
- Date: 2026-09-07

## Context

Before 0.3, Lattice exposed only the foundation metadata command, but the desktop shell had no production CSP, broad default capability grants, custom command exposure without explicit app-manifest ACL registration, and startup failure handling that could report an error without forcing an unsuccessful process status.

Tauri v2 documentation and local source inspection on 2026-09-07 showed that CSP is injected for bundled non-dev assets only when configured, and that application commands require `tauri_build::AppManifest::commands(...)` before app command ACL is enforced.

## Decision

Lattice configures a restrictive production CSP for bundled local assets and required Tauri IPC. The policy forbids `unsafe-eval`, wildcard remote script/connect sources, frames, objects, and production dev-server origins.

Angular component styles currently require a narrow `style-src 'unsafe-inline'` allowance because the compiled Angular shell injects component CSS at runtime. This exception is limited to styles; scripts keep `'self'` only.

The main window is explicitly labeled `main`. Its only current application permission is `allow-get-app-info`, generated from the Rust-owned `GET_APP_INFO_COMMAND` through the Tauri app manifest in `build.rs`. Broad `core:default` and `log:default` WebView grants are removed until a current UI consumer justifies individual permissions.

Fatal desktop startup errors now exit with status code 1 after printing a startup failure message to stderr.

## Consequences

- Future native capabilities start from a deny-by-default desktop surface.
- Remote UI and production dev-server authority are not implicitly trusted.
- Custom command exposure is tied back to the 0.2 Rust-owned command inventory.
- The style exception must be revisited if Angular/Tauri support allows component styles without inline style authority.
- Full 0.3 completion still requires recorded native WebView denial evidence and P0 idle resource measurements.
