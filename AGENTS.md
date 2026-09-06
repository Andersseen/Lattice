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

## Forbidden Shortcuts

- Do not bypass type safety.
- Do not add dependencies without justification.
- Do not introduce business logic in Angular when it belongs in Rust.
- Do not couple the application directly to llmster APIs outside its adapter.
- Do not implement speculative future features.
- Do not weaken tests to make CI green.
- Do not replace existing architecture without an ADR/spec when the change is significant.
- Do not create placeholder abstractions with no current consumer.
