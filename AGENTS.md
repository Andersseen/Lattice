# Agent Instructions

This repository is the Lattice foundation. Treat it as a lightweight desktop app, not as a place to prebuild future agent features.

## Architecture

- Angular owns presentation, routing, and interaction state.
- Rust owns native capabilities and future process/storage/security/runtime work.
- Angular must communicate with Rust through typed application APIs and Tauri IPC.
- Shared TypeScript contracts live in `packages/types`.
- Current Rust core code lives in `crates/lattice-core`.
- Do not let Angular call llmster, Hermes, OpenAI, Anthropic, Gemini, Wisp, Vertex, MCP transports, filesystem APIs, terminal APIs, or secrets APIs directly.

## Commands

```bash
pnpm dev
pnpm build
pnpm build:web
pnpm lint
pnpm typecheck
pnpm test
pnpm e2e
pnpm check
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Angular Conventions

- Use standalone APIs.
- Use zoneless change detection.
- Prefer signals and `computed`.
- Use `effect` only when side effects are actually needed.
- Use modern Angular control flow.
- Prefer feature/capability folders over global dumping grounds.
- Do not introduce NgModules, NgRx, Redux, or global state managers without an accepted spec.

## Rust Conventions

- Keep `unsafe` out of the project.
- Avoid `unwrap()` and `expect()` in productive code.
- Use typed errors where the boundary benefits from structure.
- Keep native access behind Rust APIs.
- Treat Clippy warnings seriously.

## Testing Requirements

- TypeScript behavior should have focused Vitest tests.
- Rust behavior should have Cargo tests.
- UI smoke behavior should be covered by Playwright where reliable.
- Do not add tests that only assert default framework boilerplate.
- Do not weaken tests to make CI green.

## OpenSpec Workflow

Use OpenSpec for meaningful product or architecture changes:

```text
explore -> propose -> spec -> design -> tasks -> implementation -> verification -> archive
```

Significant changes should link to an OpenSpec change in the PR.

## Implementing A Roadmap Minor

- Read `docs/architecture.md`, `docs/roadmap.md`, its linked v1 constraints, and the current `openspec/specs/` before implementation.
- Implement only the requested minor after its prerequisites are verified and its OpenSpec proposal, behavioral deltas, design and tasks are accepted.
- The roadmap describes future scope; it does not make future contracts implemented or authorize adjacent minors. Keep one coherent release objective per branch/PR.
- Use the minor's acceptance checklist and the common Definition of Done in `docs/v1/definition-of-v1.md`; record negative-case, native and resource evidence where required.
- Update current specs and capability status only to match verified behavior, then archive the completed change. Do not open all future changes or prebuild their abstractions.
- Every new dependency needs a current consumer, alternative considered, runtime cost, maintenance/license evidence and security implications in the design.

## Forbidden Shortcuts

- Do not bypass type safety.
- Do not add dependencies without justification.
- Do not introduce business logic in Angular when it belongs in Rust.
- Do not couple the application directly to llmster APIs outside its adapter.
- Do not implement speculative future features.
- Do not weaken tests to make CI green.
- Do not replace existing architecture without an ADR/spec when the change is significant.
- Do not create placeholder abstractions with no current consumer.
