# Lattice

A lightweight, local-first workspace for models, agents, tools, and skills.

Lattice is currently under active early development. This repository is a foundation: it creates the desktop shell, typed application boundary, quality tooling, documentation, and OpenSpec workflow needed to grow the product incrementally.

## What Is Lattice?

Lattice is planned as a local-first desktop application for running agentic workflows from one provider-agnostic workspace. The long-term product direction includes chat, workspaces, skills, memory, MCP, tasks, model runtime management, and optional integrations.

Implemented now, on the active `0.8` branch (evidence limits recorded in `docs/verification/`, not yet archived into permanent OpenSpec specs):

- Tauri 2 desktop shell with a restrictive production CSP and explicit command/window permissions (`0.3`).
- Angular zoneless frontend with standalone APIs, signals, lazy routes, strict TypeScript settings, and strict template checking.
- Typed Angular application API backed by generated Rust-owned wire contracts, with a checked Tauri IPC command inventory and a browser fallback for every capability below.
- Rust-owned SQLite settings with versioned migrations, backups and revision conflicts (`0.4`).
- Replaceable `ModelRuntime` discovery and owned/attached start-stop lifecycle for llmster, behind an isolated adapter (`0.5`–`0.6`).
- Installed/loaded local model inventory with a single managed load slot (`0.7`).
- A provider-neutral local completion port with streaming chat, cancellation and a model lease, backed by a local OpenAI-compatible (llmster) adapter (`0.8`).
- pnpm, Turborepo, and Cargo workspaces.
- Vitest, Playwright, ESLint, Prettier, rustfmt, Clippy, and CI configuration.
- OpenSpec-driven development for every change above.

Not yet implemented:

- Conversation persistence (reopening/continuing chats after restart) and remote/Anthropic/Gemini providers.
- Native OS-secure credential storage.
- Workspace filesystem scope, a bounded agent loop, tool execution (read/write/terminal), portable skills, MCP, explicit memory, Spaces, and tasks/scheduling.
- Signed/notarized packaging and first-use qualification.

Exploratory:

- Optional Vertex and Wisp integrations.
- Native desktop automation beyond the current web-shell Playwright smoke tests; native GUI interaction with the packaged app is recorded as pending evidence per minor, not automated yet.

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
- No real agent runtime, tool execution, MCP, skills, memory, tasks, spaces, or Kanban yet.
- No remote provider integrations (OpenAI-compatible remote, Anthropic, Gemini), model downloading/marketplace, Wisp, or Vertex yet.

## Architecture Overview

Every capability follows the same shape; the simplest example is the original foundation path:

```text
Angular home page
  -> AppApiService
  -> generated command inventory
  -> Tauri invoke("get_app_info")
  -> lattice-desktop command
  -> lattice-core::app_info()
```

Settings, runtime discovery/lifecycle, local models and chat streaming each add their own typed commands/store behind this same boundary; none of them let Angular call llmster, SQLite, or process APIs directly.

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
- UI foundation: Volt UI themes/components, Angular Movement, Lumen Icons, and Quartz Headless primitives.
- Agent development tooling: Agentyx project-local pack configuration for Codex skills/MCP planning.

Volt UI, Angular Movement, Lumen Icons and Quartz Headless are admitted as frontend foundations with small current consumers. Agentyx is admitted as development tooling only; it does not add product skills or MCP runtime behavior.

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
pnpm agentyx:doctor
pnpm agentyx:install:dry-run
pnpm agentyx:install
```

`pnpm contracts:generate` refreshes committed TypeScript bindings from Rust-owned wire contracts. `pnpm contracts:check` verifies that the committed bindings match Rust.

`pnpm agentyx:doctor` verifies the project-local Agentyx configuration. `pnpm agentyx:install:dry-run` previews skill/MCP installation plans, and `pnpm agentyx:install` applies them to project-local provider files.

Useful Rust commands:

```bash
cargo fmt --all --check
cargo check --workspace
cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
```

## Testing

- Vitest covers TypeScript bridge/fallback/store behavior, including a browser-simulated streaming chat fallback.
- Cargo tests cover `lattice-core` use cases (settings, runtime discovery/lifecycle, local models, chat streaming) via disposable process/loopback fixtures, plus generated-contract drift checks.
- Playwright drives the real web shell end to end: settings, runtime configure/start/stop, model load/unload, and a full chat send/stream/cancel journey, all against the browser fallback.

Native desktop automation (a packaged Tauri window, or a real llmster backend) is intentionally deferred per minor and recorded as pending evidence in `docs/verification/`, not automated in CI. Current E2E verifies application/UI behavior against the browser fallback; Rust tests verify native core and IPC command behavior against disposable fixtures, not a live llmster install.

## Repository Structure

```text
apps/
  desktop/              Angular frontend and Tauri shell
crates/
  lattice-core/          Rust application core (settings, model runtime, providers)
packages/
  types/                 Shared TypeScript API contracts, generated from Rust
e2e/                     Playwright web-shell smoke tests
docs/                    Architecture, ADRs, roadmap, development workflow, verification records
openspec/                OpenSpec config, active changes and accepted specs
.github/                CI, issue templates, PR template
```

## Roadmap

See [docs/roadmap.md](docs/roadmap.md). The roadmap has no dates and does not make Wisp or Vertex part of v1.

The [repository assessment](docs/v1/repository-assessment.md) distinguishes implemented behavior from planned releases; the [Definition of 1.0](docs/v1/definition-of-v1.md) sets the required workflow and platform support. The next implementation scope is **0.9 — Conversation persistence**.

## Contributing

See [CONTRIBUTING.md](CONTRIBUTING.md). Use Conventional Commits and keep significant behavior or architecture changes tied to OpenSpec.

## Security

See [SECURITY.md](SECURITY.md). Do not store credentials in frontend state, localStorage, or committed files.

## License

MIT. See [LICENSE](LICENSE).
