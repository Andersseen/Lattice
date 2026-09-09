# llmster Lifecycle Design

## Ownership

`lattice-core` keeps owning the ModelRuntime contract: ownership determination, argv construction, health-wait timing, storage schema and safe errors. `lattice-desktop` (the Tauri crate at `apps/desktop/src-tauri`) exposes typed commands only and holds the shared cancellation flag; it has no independent process or network authority. Angular owns only Models control presentation (start/stop buttons, pending/owned/attached/error states) and calls the typed application API, exactly as in 0.5.

`crates/lattice-core/src/model_runtime.rs` (922 lines) is split into a module directory so discovery and lifecycle concerns stay separable while keeping one capability spec:

- `crates/lattice-core/src/model_runtime/mod.rs` — shared types (`ModelRuntimeAvailability`, `Version`, error re-exports) and the public surface re-exported from `lattice-core::lib`.
- `crates/lattice-core/src/model_runtime/discovery.rs` — unchanged 0.5 content (probe adapter, `ModelRuntimeStatus`, `validate_runtime_executable_path`, `executable_fingerprint`).
- `crates/lattice-core/src/model_runtime/lifecycle.rs` — new: `RuntimeOwnership`, `RuntimeOperationOutcome`, `start_model_runtime`, `stop_model_runtime`, argv construction, health wait, shutdown hook entry point.

No new crate and no new workspace dependency. See "Dependency admission" below.

## Contracts

Three new Tauri commands, all requiring `expected_revision` for the same optimistic-concurrency pattern 0.5 already uses (`AppError::runtime_conflict` on mismatch):

```rust
pub const START_MODEL_RUNTIME_COMMAND: &str = "start_model_runtime";
pub const STOP_MODEL_RUNTIME_COMMAND: &str = "stop_model_runtime";
pub const CANCEL_MODEL_RUNTIME_OPERATION_COMMAND: &str = "cancel_model_runtime_operation";

pub struct StartModelRuntimeRequest { pub expected_revision: u64 }
pub struct StopModelRuntimeRequest { pub expected_revision: u64 }
pub struct CancelModelRuntimeOperationRequest {} // no payload; cancels whichever operation is in flight

pub enum RuntimeOwnership {
    Owned { daemon_pid: u32, executable_fingerprint: String, owned_since_unix_seconds: u64 },
    Attached,
    Unknown,
}

pub enum RuntimeOperationOutcome {
    Started,
    AlreadyRunning,
    Stopped,
    AlreadyStopped,
    Refused, // stop requested on attached/unknown resource; nothing was executed
    Cancelled,
    TimedOut,
    Failed,
}
```

`ModelRuntimeStatus` (0.5) gains two fields: `ownership: RuntimeOwnership` and `last_operation: Option<RuntimeOperationOutcome>`. Both commands return the full refreshed `ModelRuntimeStatus`, matching the existing `get_model_runtime_status`/`probe_model_runtime` return shape — no separate operation-result type, so the frontend always renders from one source of truth.

`start_model_runtime` and `stop_model_runtime` are plain synchronous `#[tauri::command] fn`s, exactly like every other command in this codebase — Tauri already dispatches non-`async` commands onto its own blocking thread pool, off the main/webview thread, so a call that blocks synchronously for up to 90 seconds via the same `thread::sleep` poll loop the 0.5 probe already uses needs no `async`/await and no new runtime dependency. `cancel_model_runtime_operation` is a third, separate synchronous command that only flips a shared flag and returns immediately; it never touches the settings mutex (see "Health wait and cancellation"), so it is never blocked behind an in-flight start/stop.

## Ownership determination

Ownership is never assumed from a single observation. Before issuing any mutating command, `start_model_runtime`:

1. Re-probes current daemon/server status exactly as `probe_model_runtime` does (same argv, same timeout).
2. If the daemon is already reported running:
   - If a persisted ownership record exists whose `daemon_pid` and `executable_fingerprint` both match the current observation, restore `Owned` and treat the call as idempotent (`AlreadyRunning`, no process spawned).
   - Otherwise mark `Attached` and return `AlreadyRunning` without spawning anything. This covers both a GUI-launched instance and a daemon started outside any Lattice session.
3. If the daemon is not running, spawn `lms daemon up --json` with fixed argv, no shell, sanitized environment (matching the 0.5 probe adapter's environment handling). Its own stdout is not parsed for success or identity: the real-CLI check below found it can exit `0` while printing a plain-text failure and no JSON at all, so it is treated purely as a fire-and-wait mutating call.
4. Immediately follow with a `daemon status --json` probe (the same call the 0.5 adapter already uses and already fully trusts) and take its reported `pid` as the daemon identity. This is the "reconcile daemonization" requirement satisfied without a second bespoke parser: the confirming probe, not the short-lived `lms daemon up` invocation's own exit code or PID, is the sole source of truth for both success and identity.
5. If a server is requested and not already reachable, spawn `lms server start --bind 127.0.0.1` (port omitted so llmster reuses its last configured port; never `--cors`, never a non-loopback `--bind`), then perform the existing 0.5 loopback TCP health wait against the port `server status --json --quiet` reports.

Ownership is tracked at the daemon level, not separately per daemon/server: the daemon is the resource with a real process identity and the one llmster itself partially protects (`daemon down` already refuses a GUI-owned instance). If the daemon is found already running before a start call (`Attached`), `start_model_runtime` does not attempt to start the server either, even if the server is down — an attached daemon's server configuration is not Lattice's to change. The server is only started as a direct consequence of Lattice having just confirmed it owns the daemon, and is only stopped alongside stopping that same owned daemon.

`stop_model_runtime` re-probes first as well, and only issues `lms server stop` / `lms daemon down` when the current observation still matches the persisted `Owned` record's PID and fingerprint. Any mismatch, `Attached`, or `Unknown` ownership returns `Refused` without executing a mutating command — this is the direct implementation of "no kill-by-name or attached-resource stop." `lms daemon down` itself additionally refuses to stop a GUI-launched instance (confirmed 2026-09-09 against current docs, see Documentation sources), which is a second, independent backstop behind Lattice's own ownership check, not a replacement for it.

Because llmster does not expose a process start-time API, a stored `Owned` record surviving a Lattice restart is accepted only on exact PID and executable-fingerprint match, with the known residual risk of PID reuse after a host reboot. That risk is not closed with a new dependency in 0.6; it is recorded as a documented limitation (see "Dependency admission") and revisited only if real usage shows it matters.

## Health wait and cancellation

Both start and stop share one bounded poll loop, extending the existing `run_probe_command` pattern (fixed deadline via `Instant`, 20 ms poll interval, capped output) rather than adding an async runtime:

- Deadline: 90 seconds total per start operation (daemon confirmation and, if requested, server loopback health combined); 10 seconds for stop (an already-owned process shutting down is not expected to be slow). See "Real CLI behavior" below for why start needs a much longer bound than originally assumed. This stays a hard ceiling, not a target: the UI shows a pending state and offers cancellation for the whole wait, per the next requirement.
- `DesktopState` gains `runtime_operation_cancelled: Arc<AtomicBool>`, reset to `false` at the start of every start/stop call and checked once per poll tick.
- `cancel_model_runtime_operation` sets that flag. The in-flight command observes it, stops polling, and returns `Cancelled` with a status derived from one final probe — it never attempts to undo a daemon or server that already reports running, because tearing down a resource outside the verified ownership check above would itself be an unaccounted-for stop. Cancelling only ever shortens the wait, never forces an extra mutation.
- Exceeding the applicable deadline without cancellation returns `TimedOut` with the same final-probe-derived status; a `lms` invocation subprocess still running at that point is killed the same way `run_probe_command` already kills a hung probe.

This keeps 0.6 within `std::process` + `std::thread` + `std::sync::atomic`, matching the codebase's zero-async-runtime baseline.

## Shutdown

`apps/desktop/src-tauri/src/lib.rs` registers a `tauri::RunEvent::ExitRequested` (or the equivalent `run` callback) handler that, if the in-memory status shows `Owned`, calls the same stop path with a short fixed deadline (2 seconds, matching the existing probe timeout rather than the full 10 second interactive deadline) before allowing exit to proceed. Failure to stop in time is logged through the existing `tauri-plugin-log` sink and does not block application exit — "failure is visible," not "failure is fatal." An `Attached` or `Unknown` resource is never touched at shutdown.

## Storage

Settings schema migrates v2 → v3 following the exact `migrate()` transactional/backup pattern already implemented in `crates/lattice-core/src/settings.rs`: a new pre-migration backup, one transaction, `PRAGMA user_version` bump, refusal of any schema newer than 3. The `model_runtime_discovery` row gains three nullable columns: `ownership_state` (`owned` | `attached` | `unknown`), `owned_daemon_pid`, `owned_since_unix_seconds`; the existing `executable_fingerprint` column (already present from 0.5 approval tracking) is reused as the ownership match key instead of adding a duplicate column. On read, `with_current_file_state()` (0.5) is extended so a fingerprint change also downgrades `ownership_state` to `unknown`, alongside its existing approval-clearing behavior.

## UI

The Models view (0.5) gains Start/Stop controls reflecting `ownership` and `last_operation`: a `Start` action is offered only when availability is `stopped`/`missing`-is-configured; `Stop` is offered only when `ownership` is `owned` and disabled (with an explanatory label, not hidden) when `attached`/`unknown`. A pending state covers the async command's flight time and exposes the `cancel_model_runtime_operation` action. No new route; this extends the existing Models component and store from 0.5.

## Dependency admission

Considered: `sysinfo` (to verify process identity/start-time more robustly across restarts) and a job-control crate for process-group termination. Rejected for 0.6: 0.5 solved a structurally identical problem (bounded external process invocation, capped output, timeout-kill) with zero new dependencies, `unsafe_code = "forbid"` applies workspace-wide so any candidate crate would need to be pure-safe-Rust or carry justified `#[allow]`s, and the one gap `std` cannot close — verifying a PID was not reused after a full host reboot — is documented as a residual, low-probability limitation rather than a purchased dependency. This will be revisited only if P1/real-runtime evidence (see Verification) surfaces an actual false-positive ownership claim.

## Documentation sources

Checked 2026-09-09 against current `lmstudio.ai` docs:

- `https://lmstudio.ai/docs/cli/daemon/daemon-up` — `lms daemon up [--json]`; idempotent ("if already running, reports current status and displays the process ID"); `--json` output shape `{ "status": "running", "pid": <num>, "isDaemon": true, "version": "<semver>" }`, which is the daemon PID this design trusts as identity.
- `https://lmstudio.ai/docs/cli/daemon/daemon-down` — `lms daemon down`; "only works if llmster is running. It will not stop LM Studio if it is running as a GUI app." — confirms the CLI's own attached-resource guard for the daemon layer.
- `https://lmstudio.ai/docs/cli/serve/server-start` — `lms server start [--port <n>] [--cors] [--bind <addr>]`; `--bind` defaults to `127.0.0.1`; this design pins `--bind 127.0.0.1` explicitly and never sets `--cors` or `--port`.
- `https://lmstudio.ai/docs/cli/serve/server-stop` — `lms server stop`; "gracefully stops the running LM Studio server," terminates active requests; no documented GUI-attachment guard at this layer, which is why Lattice's own ownership check in "Ownership determination" is the sole guard for the server resource, not a backstop.

Undocumented and therefore not relied upon: `daemon down` JSON output shape and exit codes, `server start`/`server stop` JSON output and preconditions. Every mutating call is always followed by an independent status probe rather than trusting its own stdout for anything beyond the daemon's reported PID.

### Real CLI behavior (2026-09-09, installed CLI commit `71bd99c`)

Confirmed by direct invocation on the development machine, not only documentation:

- `lms daemon --help` / `lms server --help` confirm exact subcommand names and flags assumed above (`daemon up [--json]`, `daemon down` with no flags, `daemon status [--json]`, `server start [-p/--port] [--bind] [--cors]`, `server stop` with no flags, `server status [--json]`).
- `lms daemon up --json` did **not** behave as the public docs' example JSON output implied. Two consecutive live attempts on this machine (LM Studio.app installed but never launched) each printed plain text — `Waking up LM Studio service...` followed by `Error: Timed out waiting for LM Studio daemon to start.` and `Error: Failed to start or connect to local LM Studio API server.` — with **no JSON emitted at all**, and each attempt took roughly 60 seconds before giving up. Both attempts **exited with status code 0** despite reporting failure in text. `lms daemon status --json` immediately after each attempt correctly reported `{"status":"not-running"}` with no orphaned process observed in `ps aux`.
- This changes two design decisions from an earlier draft: the start deadline must accommodate a slow/failed wake attempt (raised from an initially assumed 10 seconds to 90 seconds, see "Health wait and cancellation"), and success/failure of `daemon up` must never be read from its exit code — Lattice already only trusts the reported `pid` when it can parse `--json` output as the documented shape, and always confirms with a follow-up `daemon status --json` probe regardless, so this real failure mode requires no further design change beyond the deadline.
- Root cause is environmental, not a Lattice concern: llmster's headless wake path on this machine most likely requires LM Studio.app to have been opened at least once to complete first-run registration (the CLI daemon docs describe this as installed via LM Studio's own installer). This is out of scope to fix or work around in 0.6 — Lattice reports whatever `daemon status`/`daemon up` actually return, including this failure, as an honest `Failed`/`TimedOut` outcome with the same message surfaced by the probe, and documents this as a known local-setup prerequisite in the eventual user-facing troubleshooting guide.

## Verification

Testing tier follows the 0.5–0.8 row of the Definition of v1 testing table: disposable fixture CLI/process scripts for PR-time checks (duplicate start, occupied endpoint/port, timeout, crash mid-start, already-daemonized identity, attached resource, cancellation at each phase, shutdown-hook stop), and an opt-in real llmster smoke test without loading a model, recorded as skipped (never passed) when the prerequisite is absent.

P1 profile (`docs/verification/0.6-llmster-lifecycle.md`, produced during the Verify step, not part of this design): ten start/stop cycles comparing no-runtime, attached-runtime (a manually pre-started GUI/CLI instance), and owned-idle-runtime memory/process baselines; confirms zero orphaned or duplicate daemon after each cycle and after final exit, per the Definition of v1 P1 gate.
