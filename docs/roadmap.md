# Roadmap

This roadmap describes sequence, not dates. Each significant phase should move through OpenSpec before implementation.

## Phase 0 - Foundation

Implemented in this setup:

- pnpm, Turborepo, and Cargo workspaces.
- Tauri 2 desktop shell.
- Angular zoneless frontend.
- Minimal Rust core.
- Typed IPC smoke path.
- Testing, linting, formatting, and CI.
- OSS project files.
- OpenSpec and SDD workflow documentation.

## Phase 1 - Application Core

- Typed IPC conventions.
- Settings.
- Persistence.
- Runtime lifecycle.
- Process management.
- Basic permissions model.

## Phase 2 - Model Runtime

- `ModelRuntime` abstraction.
- llmster adapter.
- Runtime detection.
- List local models.
- Load and unload model.
- Basic local inference.

## Phase 3 - Agent Core

- Provider abstraction.
- Streaming responses.
- Basic agent loop.
- Tool registry.
- Conversation persistence.

## Phase 4 - Agent Capabilities

- Skills.
- MCP client.
- Basic memory.
- Permission prompts.
- Safer terminal and filesystem boundaries.

## Phase 5 - Workspace

- Spaces.
- Workspace context.
- Tasks.
- Scheduled execution.

## Phase 6 - v1 Hardening

- Resource profiling.
- Security review.
- Reliability work.
- UX polish.
- Packaging.
- Documentation.
- Cross-platform testing.

## Post-v1 / Optional

- Vertex integration.
- Wisp integration.
- External agent interop.
- Advanced native desktop automation.
