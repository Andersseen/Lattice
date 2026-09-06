# Lattice

A lightweight, local-first workspace for models, agents, tools, and skills.

Lattice is currently under active early development. This repository is a foundation: it creates the desktop shell, typed application boundary, quality tooling, documentation, and OpenSpec workflow needed to grow the product incrementally.

## What Is Lattice?

Lattice is planned as a local-first desktop application for running agentic workflows from one provider-agnostic workspace. The long-term product direction includes chat, workspaces, skills, memory, MCP, tasks, model runtime management, and optional integrations.

Implemented now:

- Tauri 2 desktop shell.
- Angular zoneless frontend with standalone APIs, signals, lazy routes, strict TypeScript settings, and strict template checking.
- Typed Angular application API that calls a generated Tauri IPC command name when running in desktop mode.
- Rust core crate with a minimal `get_app_info` smoke path.
- pnpm, Turborepo, and Cargo workspaces.
- Vitest, Playwright, ESLint, Prettier, rustfmt, Clippy, and CI configuration.
- OpenSpec initialization and project documentation.

Planned:

- Settings and persistence owned by Rust.
- Runtime lifecycle and process management.
- A replaceable model runtime abstraction, initially backed by llmster.
- Provider abstraction, tool registry, basic agent loop, skills, MCP, memory, permissions, and workspace context.
- Required native workflow verification and resource profiling before 1.0.

Exploratory:

- Optional Vertex and Wisp integrations.
- Native desktop automation beyond the current web-shell Playwright smoke tests.

## Why Lattice?

Local AI desktop software can become heavy quickly: persistent Node services, broad provider coupling, eager background processes, and UI layers that know too much about native capabilities. Lattice is designed around a smaller boundary:

```text
Angular UI
  -> typed application API
  -> Tauri IPC
  -> Rust application core
  -> future adapters
```

The goal is to keep Lattice's own runtime overhead small enough that the local model gets most of the machine's memory.

## Project Status

Pre-1.0 foundation. Do not treat future capabilities in the docs as implemented product behavior. They are targets for later OpenSpec-backed changes.

## Goals

- Local-first by default.
- Low application overhead that leaves memory available for local models; resource targets require measurement.
- Provider-agnostic prompts, tools, skills, memory, and runtime boundaries.
- Rust-owned native capabilities and future OS/process/storage/security code.
- Angular-owned presentation and interaction state.
- Small, testable changes driven by OpenSpec when behavior or architecture changes significantly.

## Non-Goals

- No Electron.
- No persistent Node/Bun runtime for app infrastructure.
- No real agent runtime yet.
- No local inference, llmster integration, MCP, skills, memory, tasks, spaces, Kanban, Wisp, Vertex, provider integrations, model downloading, or model management yet.

## Architecture Overview

The current smoke path is intentionally small:

```text
Angular home page
  -> AppApiService
  -> generated command inventory
  -> Tauri invoke("get_app_info")
  -> lattice-desktop command
  -> lattice-core::app_info()
```

In browser-only tests, the same application API returns a typed fallback so CI can verify the web shell without desktop automation. Native IPC responses are decoded from untrusted payloads before they enter application state.

See [docs/architecture.md](docs/architecture.md).

## Technology Stack

- Desktop: Tauri 2 and Rust stable.
- Frontend: Angular 22, standalone APIs, zoneless change detection, signals, modern control flow, lazy routes.
- Package management: pnpm workspaces.
- Task orchestration: Turborepo.
- Native code: Cargo workspace.
- Testing: Vitest, Playwright, Cargo tests.
- Quality: ESLint flat config, Prettier, rustfmt, Clippy.

Volt UI and Angular Movement are not installed yet. They are expected frontend integrations, but this setup keeps styling simple until their real API and dependency requirements are introduced by a focused change.

## Getting Started

Requirements:

- Node.js 22.22.3 or newer in the Node 22 line.
- pnpm 10 via Corepack.
- Rust stable.
- Tauri 2 platform prerequisites for your operating system.

```bash
corepack enable
pnpm install
pnpm dev
```

`pnpm dev` starts the Tauri desktop app and the Angular dev server.

## Development

```bash
pnpm dev
pnpm build
pnpm build:web
pnpm lint
pnpm typecheck
pnpm test
pnpm e2e
pnpm check
pnpm contracts:generate
pnpm contracts:check
```

`pnpm contracts:generate` refreshes committed TypeScript bindings from Rust-owned wire contracts. `pnpm contracts:check` verifies that the committed bindings match Rust.

Useful Rust commands:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Testing

- Vitest covers small TypeScript bridge behavior.
- Cargo tests cover `lattice-core` app info and structured IPC error serialization.
- Playwright covers the web shell, initial route, and navigation.

Native desktop automation is intentionally deferred. Current E2E prioritizes reliable CI verification of the frontend shell, while Rust tests verify the native core and IPC command behavior.

## Repository Structure

```text
apps/
  desktop/              Angular frontend and Tauri shell
crates/
  lattice-core/          Minimal Rust application core
packages/
  types/                 Shared TypeScript API contracts
e2e/                     Playwright web-shell smoke tests
docs/                    Architecture, ADRs, roadmap, development workflow
openspec/                OpenSpec config and future spec source of truth
.github/                CI, issue templates, PR template
```

## Roadmap

See [docs/roadmap.md](docs/roadmap.md). The roadmap has no dates and does not make Wisp or Vertex part of v1.

The [repository assessment](docs/v1/repository-assessment.md) distinguishes implemented behavior from planned releases; the [Definition of 1.0](docs/v1/definition-of-v1.md) sets the required workflow and platform support. The next implementation scope is **0.2 — Application Runtime Foundation**.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Use Conventional Commits and keep significant behavior or architecture changes tied to OpenSpec.

## Security

See [SECURITY.md](SECURITY.md). Do not store credentials in frontend state, localStorage, or committed files.

## License

MIT. See [LICENSE](LICENSE).
