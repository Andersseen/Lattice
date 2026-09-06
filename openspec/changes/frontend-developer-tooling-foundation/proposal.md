# Frontend and Developer Tooling Foundation

## Objective

Admit the requested UI and agent-development libraries with small current consumers and clear boundaries.

## Rationale

Lattice will need a richer Angular UI surface before settings, model runtime, workspace and skills work. Adding the frontend libraries now is acceptable only if each has a real foundation consumer and passes existing checks. Agentyx can help manage project-local skills/MCP configuration for contributors, but must remain development tooling until Lattice's own skills and MCP product capabilities are implemented through later roadmap minors.

## Scope

- Add Volt UI components/theme as an application dependency and wire its theme provider/CSS.
- Add Angular Movement as an application dependency and use a stable directive in the existing home page.
- Add Lumen Icons as an application dependency and use one tree-shakeable icon import in the shell.
- Add Quartz Headless as an application dependency and use one accessible headless tooltip directive.
- Add Agentyx CLI as a root development dependency with project-local config and dry-run/doctor scripts.

## Non-goals

- No product skills, MCP runtime, tool registry, agent loop, provider runtime, local model integration, or persistence behavior.
- No global Agentyx installation and no automatic provider-file writes.
- No broad UI redesign or component migration.
- No unscoped copy-paste component import from external CLIs.

## Impacted Capabilities

- `foundation`
- `application-api`
- `testing-ci-sdd`
- future `skills` and `mcp` only as development planning context, not product behavior
