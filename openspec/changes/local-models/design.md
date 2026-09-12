# Installed Model Management Design

## Ownership

`lattice-core` keeps owning the model contract: ownership determination, argv construction, confirmation-wait timing, storage schema and safe errors. `lattice-desktop` exposes typed commands only and holds the shared cancellation flag, exactly as 0.6 already does for runtime lifecycle. Angular owns only Models list/load/unload presentation and calls the typed application API.

This change adds one new file to the existing module directory rather than a new crate or top-level module, because model management is still the `ModelRuntime` boundary the roadmap and capability map assign it to:

- `crates/lattice-core/src/model_runtime/models.rs` — new: `ModelDescriptor`, `LoadedModelObservation`, `ModelLoadOwnership`, `ModelOperationOutcome`, `ModelSlotStatus`, `list_installed_models`, `list_loaded_models`, `load_model`, `unload_model`.
- `crates/lattice-core/src/model_runtime/discovery.rs` and `lifecycle.rs` — unchanged. Model commands only ever read the current `ModelRuntimeStatus.availability`/`ownership` to refuse operating against a runtime that is not `running`; they do not call into `lifecycle.rs`'s start/stop paths.
- `crates/lattice-core/src/model_runtime/mod.rs` — re-exports the new public surface alongside the existing discovery/lifecycle types.

No new crate and no new workspace dependency; see "Dependency admission".

## Contracts

Four new Tauri commands, all requiring `expected_revision` for the same optimistic-concurrency pattern 0.5/0.6 already use (`AppError::runtime_conflict` on mismatch), operating on a slot state independent of `ModelRuntimeStatus.revision`:

```rust
pub const GET_MODEL_SLOT_STATUS_COMMAND: &str = "get_model_slot_status";
pub const LOAD_MODEL_COMMAND: &str = "load_model";
pub const UNLOAD_MODEL_COMMAND: &str = "unload_model";
pub const CANCEL_MODEL_OPERATION_COMMAND: &str = "cancel_model_operation";

pub struct GetModelSlotStatusRequest {} // always re-lists; no cached-only read

pub struct LoadModelRequest {
    pub expected_revision: u64,
    pub model_key: String, // must match a key from the most recent installed-inventory read
}

pub struct UnloadModelRequest { pub expected_revision: u64 }

pub struct CancelModelOperationRequest {} // no payload; cancels whichever load/unload is in flight

pub struct ModelDescriptor {
    pub model_key: String,           // llmster's own key/path identity, as returned by `ls --json`
    pub display_name: String,
    pub architecture: Option<String>,
    pub is_llm: bool,
    pub size_bytes: Option<u64>,
}

pub struct LoadedModelObservation {
    pub identifier: String,          // the `--identifier` Lattice assigned, or the one llmster reports for an attached model
    pub model_key: String,
    pub architecture: Option<String>,
    pub size_bytes: Option<u64>,
}

pub enum ModelLoadOwnership {
    Owned { identifier: String, model_key: String, loaded_since_unix_seconds: u64 },
    Attached { identifier: String, model_key: String },
    Unknown,
}

pub enum ModelOperationOutcome {
    Loaded,
    AlreadyLoaded,
    Unloaded,
    AlreadyUnloaded,
    Refused,   // load requested while an owned/attached/unknown model already occupies the slot, or unload requested on attached/unknown
    Cancelled,
    TimedOut,
    Failed,    // includes insufficient memory and any other confirmed non-load
}

pub struct ModelSlotStatus {
    pub revision: u64,
    pub installed: Vec<ModelDescriptor>,
    pub loaded: Option<LoadedModelObservation>, // None: nothing loaded in the runtime at all
    pub ownership: ModelLoadOwnership,          // meaningful only when `loaded` is Some; Unknown otherwise
    pub last_operation: Option<ModelOperationOutcome>,
    pub last_checked_unix_seconds: u64,
    pub message: String,
}
```

All four commands return the full refreshed `ModelSlotStatus`, matching the existing status-payload convention from 0.5/0.6 so the frontend always renders from one source of truth. Like `start_model_runtime`/`stop_model_runtime`, these are plain synchronous `#[tauri::command] fn`s — no new async runtime.

A precondition shared by `load_model` and `unload_model`: both first read the current `get_model_runtime_status` result and return `AppError::invalid_runtime` ("Start the runtime before managing models.") without invoking any `lms` subprocess when `availability != Running`. This is a fast, non-mutating check; it does not itself resolve or otherwise depend on runtime lifecycle state beyond that one read.

## Ownership determination

Directly mirrors ADR 0010's algorithm, applied to the model slot instead of the daemon:

1. `list_loaded_models` always runs `lms ps --json` fresh; there is no cached-only status read, exactly like `probe_model_runtime`.
2. If `ps` reports one loaded model:
   - If a persisted `Owned` record's `identifier` and `model_key` both match, restore `Owned`.
   - Otherwise mark `Attached`. This covers a model loaded through LM Studio's GUI, through `lms load` run outside Lattice, or left loaded by a prior Lattice session whose identity no longer matches (identifier collision is treated the same as no match: never assume).
3. If `ps` reports no loaded model, the slot is empty and ownership is `Unknown` (there is nothing to own or attach).
4. `load_model` refuses (`Refused`, no subprocess spawned) whenever step 1's fresh read already shows any loaded model, owned or attached — replacing an owned model requires an explicit prior `unload_model` call from the user, and an attached model is never evicted.
5. When the slot is confirmed empty, `load_model` spawns `lms load <model_key> --identifier <lattice-managed-identifier> -y` with fixed argv, no shell, sanitized environment, matching the 0.5/0.6 subprocess pattern. `-y` avoids the CLI's interactive multi-match prompt; `model_key` must be one already returned by the most recent `list_installed_models` read, never client-constructed, per the roadmap's "use observed model IDs, not guessed paths" constraint. `load`'s own stdout/exit code is not parsed for success: see "Real CLI behavior" below for why.
6. Immediately follow with an `lms ps --json` probe. If it reports the assigned identifier with the requested `model_key`, that observation is the sole source of truth for both success and identity, and ownership becomes `Owned` with `loaded_since_unix_seconds` set at confirmation time. If it does not, the outcome is `Failed` (or `TimedOut`/`Cancelled` per "Health wait and cancellation") and no ownership record is written — an unconfirmed load is never optimistically recorded as owned.
7. `unload_model` re-probes first as well, and only issues `lms unload <identifier>` when the current `ps` observation still matches the persisted `Owned` record's identifier and model key. Any mismatch, `Attached`, or `Unknown` ownership returns `Refused` without executing a mutating command — the direct model-layer equivalent of "no kill-by-name or attached-resource stop." A confirming `ps --json` probe after `unload` determines `Unloaded` versus `Failed`, again never trusting `unload`'s own exit code.

Because llmster's `ps` output carries no load-start-time field either, a stored `Owned` record surviving a Lattice restart is accepted only on exact identifier and model-key match, with the same accepted PID/identity-reuse-adjacent residual risk ADR 0010 already documents for the daemon — not a new risk this change introduces, and not closed by a new dependency here either.

## Health wait and cancellation

Reuses `run_bounded_command` from `discovery.rs` (already generalized in 0.6 to accept a deadline and cancellation flag) for both the mutating `load`/`unload` call and the confirming `ps --json` poll loop:

- Deadline: a load deadline wide enough to cover cold-start model weight loading, not just process spawn — set from the P2 profile's own cold-load measurement once available (task-tracked; provisionally 120 seconds, wider than 0.6's 90-second daemon-wake bound, because loading multi-gigabyte weights is expected to take longer than waking an already-installed daemon). Unload uses a short bound (10 seconds), matching 0.6's interactive stop deadline, since releasing an already-loaded model is not expected to be slow.
- `DesktopState` gains `model_operation_cancelled: Arc<AtomicBool>`, reset at the start of every load/unload call and checked once per poll tick, independent of 0.6's `runtime_operation_cancelled` flag so a model operation and a runtime operation can never cancel each other.
- `cancel_model_operation` sets that flag and returns immediately without touching the settings mutex, exactly like `cancel_model_runtime_operation`.
- Cancelling or timing out never issues a corrective `unload` for a load that may have partially succeeded server-side: the next `ps --json` probe is authoritative, and if it later shows the model loaded anyway, that model is recorded `Attached`, not `Owned` — the operation that requested it already reported `Cancelled`/`TimedOut`, so the caller who asked for it is not the one who gets to claim ownership retroactively without re-confirming intent.

## Shutdown

No new shutdown-hook behavior. 0.6's existing `stop_owned_model_runtime_for_shutdown` stops the owned daemon/server; llmster itself is expected to release loaded models as part of that daemon shutdown. This change does not add a separate best-effort `unload` call on exit, since there is no evidence an orphaned _loaded model_ (as opposed to an orphaned _process_) has any cost once its owning daemon is stopped — this is recorded as an open question for the P2/P4 resource profiles, not assumed safe without measurement.

## Storage

Settings schema migrates v3 → v4 following the exact `migrate()` transactional/backup pattern in `crates/lattice-core/src/settings.rs`: a new pre-migration backup, one transaction, `PRAGMA user_version` bump, refusal of any schema newer than 4. A new `model_load_state` row (single-row table, mirroring the existing single-row `app_settings`/runtime-discovery pattern) stores: `ownership_state` (`owned` | `attached` | `unknown`), `owned_identifier`, `owned_model_key`, `owned_since_unix_seconds`, all nullable except `ownership_state`. On read, a fresh `ps --json` reconciliation (step 2 of "Ownership determination") runs before the stored record is trusted, exactly like `with_current_file_state()` does for runtime ownership; a mismatch downgrades the stored record to `unknown` rather than deleting it silently.

## UI

The Models view (0.5/0.6) gains an installed-models list (from `ModelSlotStatus.installed`) with a per-row "Load" action, enabled only when the runtime is `running` and the slot is empty, and an "Unload" action shown when `ownership` is `Owned`, disabled with an explanatory label when `Attached`/`Unknown`. A pending state covers the load/unload command's flight time and exposes `cancel_model_operation`. This extends the existing Models component/store; no new route.

## Dependency admission

Considered: a structured JSON-schema validation crate for `lms ls --json`/`lms ps --json`, given both outputs are undocumented beyond one example. Rejected: `serde_json::from_str` into a narrowly-typed struct with `#[serde(default)]` on optional fields already gives the same fail-closed behavior (unparsable output becomes `Unknown`/`Failed`, never a panic) that 0.5's `LmsDaemonStatus`/`LmsServerStatus` structs already establish for equally-undocumented daemon/server JSON, so no new dependency is justified. No process-management crate is needed for the same reason 0.6 rejected one: `std::process`/`std::thread`/`std::sync::atomic` already cover fixed-argv spawn, bounded polling and cancellation.

## Documentation sources

Checked 2026-09-10 against current `lmstudio.ai` docs and the installed CLI's own `--help` output (commit `71bd99c`, same installation ADR 0010 checked):

- `https://lmstudio.ai/docs/cli/local-models/ls` — `lms ls [--llm] [--embedding] [--detailed] [--json]`; `--json` "outputs the list in JSON format," but the page documents no field-level schema, only human-readable example columns (name, parameters, architecture, size).
- `https://lmstudio.ai/docs/cli/local-models/ps` — `lms ps [--json]`; documents only a human-readable example (identifier, type, path, size, architecture), no JSON field names.
- `https://lmstudio.ai/docs/cli/local-models/load` — `lms load [model-key] [--gpu <ratio>] [--context-length <n>] [--ttl <seconds>] [--identifier <id>] [--estimate-only] [-y/--yes]`; **no `--json` flag is documented at all**.
- `lms unload [identifier] [--all]` (documented on the same page) — likewise **no `--json` flag documented**.
- Confirmed directly against the real installed binary: `lms load --help` and `lms unload --help` list exactly the flags above and no `--json`/output-format option of any kind, matching the public docs exactly (unlike `daemon up`, where 0.6 found the docs' example JSON shape unreliable in practice — here the docs and the real `--help` agree on the more basic fact that no machine-readable output mode exists for these two commands at all).

Undocumented and therefore not relied upon for parsing: the exact JSON field names of `ls`/`ps`, and any output of `load`/`unload` in any form. Every mutating call (`load`, `unload`) is followed by an independent `ps --json` probe rather than trusting its own stdout/exit code for success, identity, or failure reason — the model-layer equivalent of 0.6's daemon/server confirmation rule, made stricter here because unlike `daemon up` there is no documented JSON shape to even optimistically attempt to parse from the mutating call itself.

### Real CLI behavior (2026-09-10, installed CLI commit `71bd99c`)

`lms ls --json` and `lms ps --json` could not be exercised against live output in this environment: this machine's llmster daemon cannot currently be woken headlessly (the same first-run GUI-registration gap 0.6 recorded as pending, see `docs/verification/0.6-llmster-lifecycle.md`), so no installed/loaded model JSON sample was captured here. `lms ls --help`, `lms ps --help`, `lms load --help`, and `lms unload --help` were run directly and match the flags documented above exactly. Capturing one real `ls --json`/`ps --json` sample against an actual installed model, to pin the exact field names `ModelDescriptor`/`LoadedModelObservation` deserialize, is a blocking implementation task (see tasks.md), not assumed from the human-readable example columns alone.

## Verification

Testing tier follows the 0.5–0.8 row of the Definition of v1 testing table: disposable fixture CLI/process scripts for PR-time checks (installed-but-unloaded, duplicate/busy load, insufficient-memory failure, unload-in-use, externally-loaded/attached model, runtime-not-running refusal, cancellation at each phase, malformed `ls`/`ps` JSON), and an opt-in real llmster smoke test loading/unloading one real qualified candidate model, recorded as skipped (never passed) when the prerequisite runtime cannot be started in the running environment.

P2 profile (`docs/verification/0.7-installed-model-management.md`, produced during the Verify step, not part of this design): cold load, warm reload and unload timing/memory for each qualified candidate profile, with the 0.6 P1 idle-runtime baseline subtracted to isolate the model increment, per the Definition of v1 P2 gate.
