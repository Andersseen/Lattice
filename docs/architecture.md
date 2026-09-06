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
     future adapters
      ┌──────┴──────┐
      ▼             ▼
 AgentRuntime   ModelRuntime
                      │
                   llmster
```

## Current Implementation

The current code implements only a smoke path:

- Angular shell and lazy routes.
- `AppApiService` as the typed frontend boundary.
- Tauri IPC command `get_app_info`.
- `lattice-core::app_info()` returning structured app metadata.
- Structured app error shape with `code`, `message`, and `recoverable`.

This proves the path without creating fake agent, model, storage, MCP, or skill implementations.

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
  -> llmster today
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
<= ~1-2 GB

Remaining resources
-> local model
```

These are engineering targets, not current benchmark claims.

Future benchmarking should measure:

- Desktop idle memory.
- Angular webview memory.
- Rust process memory.
- Runtime adapter memory.
- Model process memory.
- CPU impact during idle, streaming, and background operations.

## Future Integrations

Vertex and Wisp are optional integrations, not core dependencies. External agents may be supported later, but Lattice's own core runtime should remain provider-agnostic and local-first.
