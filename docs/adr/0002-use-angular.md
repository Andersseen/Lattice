# 0002 - Use Angular For The Frontend

## Status

Accepted

## Context

Lattice needs a maintainable desktop UI with strong TypeScript, routing, component boundaries, and a path toward rich workflows. The project should avoid global state frameworks until there is enough product behavior to justify them.

## Decision

Use modern Angular with standalone APIs, zoneless change detection, signals, `computed`, modern control flow, and lazy routes.

## Consequences

- UI state can begin with Angular services and signals.
- Strict templates provide useful safety early.
- Zone.js is intentionally not installed.
- NgModules, NgRx, Redux, and broad global stores are non-default choices requiring justification.
