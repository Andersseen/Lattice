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
