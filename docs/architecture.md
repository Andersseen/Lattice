# Architecture

Lattice is a local-first desktop application with a deliberately narrow boundary between the UI and native capabilities.

```text
┌─────────────────────────┐
│ Angular / UI            │
└────────────┬────────────┘
             │ typed IPC
┌────────────▼────────────┐
│ Rust application core   │
└────────────┬────────────┘
             │
     future Rust use cases
      ┌──────┴───────────────┐
      ▼                      ▼
 Agent core             ModelRuntime management
      │                      │
 Provider adapters      llmster adapter
      │                      │
      └─ local/remote        llmster
```

## Current Implementation

The current code implements only a smoke path:

- Angular shell and lazy routes.
- `AppApiService` as the typed frontend boundary.
- Tauri IPC command `get_app_info`.
- `lattice-core::app_info()` returning structured app metadata.
- Structured app error shape with `code`, `message`, and `recoverable`.

This proves the path without creating fake agent, model, storage, MCP, or skill implementations.

The [source-based assessment](v1/repository-assessment.md) records verification limits and debts: manual Rust/TS DTO duplication, static Rust error messages, duplicate error normalization, disabled CSP, missing explicit strict template checking, minimal unit coverage and no installer certification. These are planned corrections, not implemented safeguards.

## Planned v1 Architecture

The [minor roadmap](roadmap.md) and [architecture sequence](v1/architecture-sequence.md) define the target and when each boundary gains its first consumer. ModelRuntime manages runtime/model lifecycle; a separate Provider port handles inference. The small Rust agent core owns the loop, context, skills, memory and permission-checked tools, including MCP. These capabilities and canonical conversations do not belong to the provider.

Rust owns SQLite persistence and OS credential integration when introduced; Angular keeps presentation/interaction state and typed API calls. Spaces compose existing records; tasks reuse the same bounded agent. v1 permits one active run and one managed loaded model, local stdio MCP and one-shot safe scheduling while the app is open. Vertex, Wisp and Agentix remain independent and optional.

The [Definition of v1](v1/definition-of-v1.md) specifies macOS/Apple Silicon support, Linux/Windows preview checks, measured resource gates and packaging/upgrade requirements. Current OpenSpec specs remain the source of implemented behavior; future roadmap contracts become normative through each accepted change.

## Target Boundaries

Angular owns:

- Presentation.
- UI interaction state.
- Routing.
- Calling typed application APIs.

Rust owns:

- Native filesystem access.
- Future SQLite/storage access.
- Process spawning and lifecycle.
- Secrets and OS APIs.
- Runtime orchestration.
- MCP transports.
- Model runtime adapters.
- Provider adapters.

The UI must not call llmster, Hermes, OpenAI, Anthropic, Gemini, Wisp, Vertex, MCP transports, or OS APIs directly.

## Principles

### Local-First

Local data and local execution should be the natural path. Remote providers may exist later, but they are adapters rather than the product center.

### Lightweight

Lattice should keep infrastructure overhead small so local models can use the remaining memory. Avoid resident processes and eager initialization unless they are justified by measured product needs.

### Provider Agnostic

Skills, tools, memory, prompts, and workflows must not belong to any one model provider.

### Modular

Capabilities should evolve independently behind real boundaries. A module should exist because something uses it now or because it protects an active boundary.

### Replaceable Infrastructure

`ModelRuntime` should be replaceable:

```text
ModelRuntime
  -> llmster as the first planned adapter
  -> another runtime tomorrow
```

llmster is expected to be an adapter, not part of the Lattice core identity.

### Lazy By Default

Expensive processes should start only when used. This matters especially for model runtimes, background tasks, MCP servers, file indexing, and future memory systems.

### Secure By Default

Future trust boundaries include:

- User.
- Angular UI.
- Rust core.
- Filesystem.
- Terminal.
- MCP.
- Model runtime.
- Remote provider.

Credentials must not be stored in frontend state or localStorage. Future credentials should use OS-secure storage through Rust-owned APIs.

### Observable

Future work should make RAM, CPU, model memory, and runtime processes measurable. This setup does not claim current benchmarks.

### No Speculative Abstractions

Create interfaces when there is a real consumer or boundary. Do not create placeholder `AgentRuntime`, `Skill`, `MCP`, or `Memory` abstractions before their specs exist.

## Resource Budgets

Reference machine:

```text
Apple Silicon
16 GB unified memory
```

Future target:

```text
Lattice application/runtime overhead
~1 GiB working target / 2 GiB release ceiling

Remaining resources
-> local model
```

These are engineering targets, not current benchmark claims.

The v1 quality contract defines exactly what is included in non-model overhead, measurement uncertainty, CPU/cleanup thresholds and gates at foundation, runtime, loaded model, agent and pre-release stages. The reference hardware is an engineering constraint, not product branding or a benchmark claim.

Future benchmarking should measure:

- Desktop idle memory.
- Angular webview memory.
- Rust process memory.
- Runtime adapter memory.
- Model process memory.
- CPU impact during idle, streaming, and background operations.

## Future Integrations

Vertex and Wisp are optional integrations, not core dependencies. External agents may be supported later, but Lattice's own core runtime should remain provider-agnostic and local-first.
