# Frontend and Developer Tooling Foundation Design

## Dependency Admission

| Package              | Version  | Scope                       | Current consumer                                                        | Notes                                                                                                                 |
| -------------------- | -------- | --------------------------- | ----------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------- |
| `@voltui/components` | `1.0.1`  | `apps/desktop` runtime      | `provideVoltTheme`, global theme CSS, refresh button                    | Declares Angular 21 peers; Angular 22 compatibility must be proven by local checks until upstream peer range expands. |
| `angular-movement`   | `1.1.0`  | `apps/desktop` runtime      | `provideMovement`, stable `MoveAnimateDirective` / `MoveHoverDirective` | Peer range includes Angular 22. Use stable directives only.                                                           |
| `lumen-icons`        | `0.2.0`  | `apps/desktop` runtime      | `LmnSparklesIcon` subpath import                                        | Declares Angular 21 peers; use subpath imports for tree shaking and verify Angular 22 locally.                        |
| `quartz-headless`    | `0.2.1`  | `apps/desktop` runtime      | `TooltipDirective`                                                      | Declares Angular 21 peers; use headless behavior only and keep styling local.                                         |
| `@angular/forms`     | `22.1.5` | `apps/desktop` runtime peer | Volt UI peer support                                                    | No form feature is introduced.                                                                                        |
| `@agentyx/cli`       | `0.5.0`  | root dev tooling            | `.agentyx.json`, `.agentyx.lock.json`, project-local skills/MCP config  | Does not run product code and writes only project-local provider files through explicit commands.                     |

## Boundaries

Angular remains the only consumer of UI libraries. Rust and Tauri are untouched by visual libraries. Agentyx remains a contributor tool for planning and installing project-local skills/MCP files; it does not define Lattice product skills or MCP runtime semantics.

The app imports only stable Angular Movement directives. Quartz is used for a small tooltip behavior to prove compatibility without introducing overlay-heavy UI. Lumen uses a per-icon subpath import.

## Risks

Volt UI, Lumen Icons and Quartz Headless currently publish Angular 21 peer ranges. This change relies on build, typecheck, lint, unit and E2E verification against Angular 22. If compatibility breaks, remove or pin replacements before expanding UI usage.
