# 0010 - llmster Lifecycle Ownership

## Status

Accepted

## Context

0.5 (ADR 0005) added read-only llmster discovery and explicitly deferred start/stop. 0.6 needs to start and stop the approved daemon and local server, but llmster's daemon can already be running as an attached GUI process, as a resource a prior Lattice session started, or as a process Lattice cannot identify at all. A wrong ownership assumption here means either failing to start a runtime the user needs, or stopping a process Lattice does not own.

Direct CLI experimentation against an installed llmster build (2026-09-09, CLI commit `71bd99c`) found the public docs incomplete in one important way: `lms daemon up --json` can exit `0` while printing a plain-text failure and no JSON at all when the daemon fails to wake (observed consistently on a machine where LM Studio.app had never been launched). Any design that trusted this command's own exit code or stdout for success would be wrong.

## Decision

Ownership is tracked at the daemon level only, as `RuntimeOwnership`: `owned` (with the daemon's own reported PID, the executable fingerprint at start time, and an owned-since timestamp), `attached`, or `unknown`. It is never inferred from a single observation:

- Before starting anything, `start_model_runtime` re-probes current state. A daemon already running is recorded `attached` unless a persisted `owned` record's PID and executable fingerprint both still match, in which case ownership is restored. Either way, nothing is spawned.
- A daemon Lattice spawns (`lms daemon up --json`, fixed argv, no shell) is confirmed exclusively through a follow-up `daemon status --json` probe — the same call the 0.5 adapter already trusts — never through the spawn command's own exit code or stdout. That probe's reported PID becomes the recorded identity.
- The local server is only started as a direct consequence of Lattice having just confirmed daemon ownership, and only stopped alongside stopping that same owned daemon. An attached daemon's server is never touched.
- `stop_model_runtime` re-probes and executes `lms server stop` / `lms daemon down` only when the current PID and fingerprint still match the persisted `owned` record; any mismatch, `attached`, or `unknown` ownership is refused outright rather than attempted. `lms daemon down` additionally refuses to stop a GUI-launched instance on its own, which is a second, independent backstop behind this check, not a replacement for it.
- Every lifecycle operation is bounded and cancellable: 90 seconds for start (well above the ~60 second wake-failure timeout observed above), 10 seconds for an interactive stop, 2 seconds for the best-effort stop attempted from the application-exit hook. Cancelling only stops waiting for confirmation; it never forces an additional start or stop call, since a resource that already exists is real state, not something to blindly unwind.
- No new dependency was added for process management. The existing `std::process`/`std::net`/`std::thread` probe-invocation pattern from 0.5 (fixed argv, temp-file-captured bounded output, poll-based timeout with kill+wait) was generalized to accept a configurable deadline and a cancellation flag, and reused as-is for the mutating `daemon up` / `daemon down` / `server start` / `server stop` calls.
- Ownership surviving an application restart relies on PID and executable-fingerprint match alone; llmster exposes no process-start-time API, so PID reuse after a full host reboot is a known, accepted residual risk rather than one closed by adding a process-inspection crate (e.g. `sysinfo`) for 0.6.

Full behavioral detail lives in `openspec/changes/llmster-lifecycle/design.md`; this ADR records the decision, not the implementation.

## Consequences

- Lattice can start and stop a local llmster runtime it provably owns, without ever stopping an attached or unidentifiable one.
- A resource observed running before any Lattice start call is permanently treated as not-Lattice's-to-stop for that ownership record, even though the CLI itself offers no reliable way to ask "who started this."
- Success/failure of a mutating CLI call is always decided by a fresh, independent probe, never by that call's own exit code or output — a requirement this ADR would not have without the real CLI behavior check above.
- PID reuse across a full host reboot could in principle cause a stale `owned` record to falsely re-match; this is accepted for 0.6 and revisited only if real usage shows it matters.
