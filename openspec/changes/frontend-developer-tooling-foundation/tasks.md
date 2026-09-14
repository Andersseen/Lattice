# Tasks

- [x] Verify requested package names and current published versions.
- [x] Add frontend libraries to `apps/desktop` with current consumers.
- [x] Add Agentyx CLI as root development tooling with project-local config and installed provider files.
- [x] Document dependency scope, peer risks and non-goals.
- [x] Verify format, lint, typecheck, tests, build, E2E and OpenSpec validation. Historical confirmation on 2026-09-12 used pnpm-era commands. Current tooling runs the equivalent gates through Bun: `bun run check`, `bun run e2e`, `bun --filter @lattice/desktop tauri:build` (`--no-bundle`), and `bun run openspec:validate`.
