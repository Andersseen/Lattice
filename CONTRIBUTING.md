# Contributing

Thanks for helping shape Lattice.

## Development Setup

```bash
corepack enable
pnpm install
pnpm check
```

Use Rust stable and install the Tauri prerequisites for your operating system.

## Workflow

- Keep changes small and focused.
- Use OpenSpec for significant behavior or architecture changes.
- Prefer one clear PR over a broad rewrite.
- Add tests where behavior changes.
- Run `pnpm check` before opening a PR.

## Conventional Commits

Examples:

```text
feat(core): add model runtime abstraction
fix(desktop): handle runtime startup failure
docs: document SDD workflow
test(core): cover provider errors
```

## Dependency Policy

Do not add dependencies without a concrete reason. Before adding one, check that it is current, stable, maintained, compatible with the stack, and not duplicating existing capability.

## OpenSpec

Use OpenSpec when a change affects product behavior, security boundaries, native capability access, persistence, providers, model runtime, agent runtime, MCP, skills, memory, tasks, or architecture.
