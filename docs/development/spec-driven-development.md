# Spec-Driven Development

OpenSpec is the source of truth for meaningful product and architecture changes.

The intended flow is:

```text
explore
-> propose
-> spec
-> design
-> tasks
-> implementation
-> verification
-> archive
```

This is not meant to be heavyweight waterfall. Small mechanical changes can remain small. Use judgment.

## When A Proposal Is Needed

Create an OpenSpec change when a change:

- Adds or changes user-visible behavior.
- Adds a major dependency.
- Changes the Angular/Rust/Tauri boundary.
- Introduces persistence, process management, credentials, MCP, model runtime, agent runtime, skills, memory, or providers.
- Changes security posture.
- Changes architecture in a way future contributors need to understand.

## When A Proposal Is Not Needed

A full spec is usually unnecessary for:

- Typo fixes.
- Formatting.
- Small test-only edits.
- CI maintenance that does not change product behavior.
- Documentation clarification that does not change architectural direction.

## Updating Specs

Keep specs behavior-focused. Explain what must happen and why, then keep implementation details in design notes or tasks when they are truly needed.

## Implementation

Implementation should follow accepted tasks. Do not broaden scope during implementation without updating the OpenSpec change.

## Verification

Every completed change should document verification. Prefer:

- `pnpm check`
- Focused unit tests.
- Playwright smoke tests for UI behavior.
- Cargo tests and Clippy for Rust behavior.

## Archiving

After a change lands, archive its OpenSpec change so current specs represent the accepted state.

## One Minor At A Time

The [roadmap](../roadmap.md) is the release scope index, supported by the capability map, architecture sequence and Definition of v1. It is planning, not proof of implemented behavior. Current `openspec/specs/` is the accepted behavioral source of truth; an accepted active change defines the deltas to implement next. If code, current specs and roadmap disagree, record the discrepancy in explore and resolve it in that change before coding. Never silently choose whichever document allows broader scope.

This planning update leaves the foundation spec unchanged and opens no implementation changes. No future behavior is promoted to a permanent spec merely because it has a release number.

For the next requested minor after the active 0.3 branch:

1. **Explore:** read AGENTS, architecture, baseline assessment, requested release and prerequisite specs/verification. For `0.4`, start from the archived `application-api` spec and the implemented desktop security baseline. Record actual code state, upstream API/protocol versions, platform assumptions and dependencies. Resolve questions needed for this minor now; a required contract must not remain a TODO for the implementing model.
2. **Proposal:** create one `openspec/changes/<minor-capability>/proposal.md` with objective, rationale, dependencies, explicit non-goals and impacted capability IDs. For example, 0.2 can use `application-runtime-foundation`. The directory name is a convention, not an instruction to create it in this session.
3. **Spec:** add behavior deltas under that change's `specs/<capability>/spec.md`, using the installed OpenSpec version's supported delta format. Give each requirement stable wording and observable success/failure/denial/cancellation scenarios where applicable. Copy no future minor into the current delta.
4. **Design:** document module/file ownership, public request/result/event semantics, state transitions, limits, trust boundaries, storage migration/rollback and dependency admission. Resolve current-minor implementation decisions; do not scaffold future consumers.
5. **Tasks:** create a numbered `tasks.md` where each task names intended files/modules, prerequisites, deliverable and verification. Link tasks to requirement/scenario IDs. Include native/manual evidence where fixtures cannot establish real behavior. Prefer tasks small enough for one focused editing session.
6. **Implement:** only after proposal/spec/design/tasks are accepted, execute the requested scope. New scope requires updating the change before code. Keep future releases closed; independent branches may exist only with explicit coordination and stable contracts.
7. **Verify:** record candidate commit, commands/results, scenario-to-test mapping, real integration versions, measurements, skipped/blocked checks and migrations. A skip is not a pass. Use the minor checklist and common DoD, not only `pnpm check`.
8. **Archive:** merge accepted deltas into permanent capability specs, archive the completed change through the pinned OpenSpec workflow, and reconcile roadmap/capability status, ADRs and docs. Verify resulting permanent specs. Do not archive unfinished behavior to clear a checklist.

## Permanent Capability Evolution

Keep `foundation` as the shell boundary. Add permanent specs only when the corresponding change is accepted and verified:

| Release entry | Capability directories after completion                                                                                                                                  |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 0.2–0.4       | `application-api`, `desktop-security`, `application-settings`, `local-storage`                                                                                           |
| 0.5–0.9       | `model-runtime`, `local-models`, `providers`, `chat-streaming`, `conversations`                                                                                          |
| 0.10–0.13     | `credentials`; incremental `providers` requirements, not one duplicated spec per vendor                                                                                  |
| 0.14–0.19     | `workspaces`, `permissions`, `agent-runs`, `tools`, `skills`, `mcp-tools`                                                                                                |
| 0.20–0.24     | `memory`, `spaces`, `tasks`, `task-scheduling`; `task-views` only if Kanban ships                                                                                        |
| 0.25–0.28     | `resource-lifecycle`, `execution-security`, `desktop-distribution`, `first-use`; consolidate existing cross-cutting invariants instead of duplicating conflicting policy |

Use one capability spec across releases, with incremental scenarios. Keep detailed measurements in verification records and durable decisions in ADRs. Current specs describe actual guarantees, including intentional unavailable/unsupported states. Product intentions and post-v1 possibilities stay in the roadmap.

## Validation And Handoff

The baseline `openspec:validate` script downloads `@latest` with `pnpm dlx`; it is not a reproducibly pinned validator. 0.2 must select/pin a compatible OpenSpec tool version, validate its generated proposal/spec/design/tasks format, validate active changes plus permanent specs in CI, and document exact commands. Do not install or alter tooling during a documentation-only planning session.

An implementation handoff must include: requested minor only; prerequisite commit/specs; accepted change path; required file/contract boundaries; explicit non-goals; requirement-to-task/test mapping; and exit checklist. The next product implementation handoff is `0.7` installed model management, once `0.6` llmster lifecycle (implemented, evidence pending per `docs/verification/0.6-llmster-lifecycle.md`) is archived. The implementing agent reports completed/blocked criteria and stops at that scope. It must not implement the next minor just because it appears next in the roadmap.

Upstream workflow reference: [OpenSpec project](https://github.com/Fission-AI/OpenSpec), consulted 2026-09-06. Repository-specific release policy above is Lattice's decision; exact CLI/artifact compatibility is verified when pinning the tool.
