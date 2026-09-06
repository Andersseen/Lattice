# ADR 0007: Admit UI and Agentyx Tooling

- Status: Accepted
- Date: 2026-09-06

## Context

The initial foundation used local CSS and no UI or motion libraries. The next visible product work will need accessible controls, iconography, restrained motion and headless UI primitives. The project also needs a way to manage contributor skills and MCP configuration without confusing that tooling with Lattice product capabilities.

## Decision

Admit `@voltui/components`, `angular-movement`, `lumen-icons`, `quartz-headless` and `@angular/forms` to `apps/desktop`. Each package has a small current consumer in the shell or home page, and Angular remains the only owner of presentation behavior.

Admit `@agentyx/cli` as a root dev dependency with `.agentyx.json`, `.agentyx.lock.json`, project-local Codex skills, `.codex/config.toml`, `pnpm agentyx:doctor`, `pnpm agentyx:install:dry-run` and `pnpm agentyx:install`. Agentyx is contributor tooling for project-local skill/MCP planning and installation. It is not a Lattice runtime dependency and does not implement product skills or MCP behavior.

Do not add the unscoped `agentyx` npm package. It is not the Andersseen Agentyx package family used here.

## Consequences

- UI work can start from real Angular libraries instead of bespoke controls.
- Angular 22 compatibility for Volt UI, Lumen Icons and Quartz Headless is verified locally because their peer ranges currently name Angular 21.
- Product skills and MCP remain future roadmap capabilities rather than being smuggled in through development tooling.
