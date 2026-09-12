# Tasks

- [x] Verify requested package names and current published versions.
- [x] Add frontend libraries to `apps/desktop` with current consumers.
- [x] Add Agentyx CLI as root development tooling with project-local config and installed provider files.
- [x] Document dependency scope, peer risks and non-goals.
- [x] Verify format, lint, typecheck, tests, build, E2E and OpenSpec validation. Confirmed 2026-09-12 as part of 0.8 hardening: `pnpm check` (format, lint, typecheck, all TS/Rust tests, `cargo check --workspace`, `cargo clippy --workspace --all-targets -- -D warnings`), `pnpm e2e`, `pnpm --filter @lattice/desktop tauri:build` (`--no-bundle`), and `pnpm dlx @fission-ai/openspec@1.12.0 validate --specs` all pass with the frontend libraries and Agentyx tooling this change added still in place.
