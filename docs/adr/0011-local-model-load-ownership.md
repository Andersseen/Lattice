# 0011 - Local Model Load Ownership

## Status

Accepted

## Context

0.6 (ADR 0010) can start and stop the llmster daemon/server it owns, but never inspects or manages what model, if any, is loaded inside that runtime. 0.8's streaming chat needs a known, loaded model to target, and the roadmap's P2 gate requires measured cold/warm load, unload and reload numbers per qualified candidate profile.

Direct CLI inspection against the installed llmster build (2026-09-10, CLI commit `71bd99c`, same installation ADR 0010 checked) found `lms load`/`lms unload` document no `--json` flag at all, in the public docs and in the real binary's own `--help` output alike. Unlike `daemon up` (which 0.6 found unreliable in practice despite a documented JSON shape), here there is no documented machine-readable output to even optimistically attempt to parse from the mutating call itself. A design that trusted either command's exit code for success, identity, or failure reason would repeat the exact mistake ADR 0010 already ruled out for the daemon, applied to a new resource type.

`lms ls --json` and `lms ps --json` could not be exercised against live output on this development machine either: the same headless-daemon-wake gap 0.6 recorded (`docs/verification/0.6-llmster-lifecycle.md`) still blocks a real JSON sample here (re-confirmed 2026-09-12; `open -a "LM Studio"` does not keep the GUI process alive in this environment, so the one-time first-run registration cannot be completed here). The exact field names `ModelDescriptor`/`LoadedModelObservation` deserialize are therefore a best-effort schema informed by the CLI's own human-readable example columns (name/parameters/architecture/size for `ls`; identifier/type/path/size/architecture for `ps`), with `#[serde(alias = ...)]` hedges on the most plausible alternate key spellings, not a verified shape. This is recorded as pending, not assumed correct.

## Decision

Ownership is tracked at the model-slot level only, as `ModelLoadOwnership`: `Owned` (with the fixed Lattice-assigned identifier, the requested model key, and a load-confirmed timestamp), `Attached`, or `Unknown` — the direct model-layer counterpart of ADR 0010's `RuntimeOwnership`, reusing the same never-assume pattern rather than inventing a new one:

- `list_loaded_models` always runs `lms ps --json` fresh; there is no cached-only status read, exactly like `probe_model_runtime` and `list_installed_models`.
- A model already loaded when Lattice first checks is `Attached` unless a persisted `Owned` record's identifier and model key both still match, in which case ownership is restored. Either way, `load_model` refuses outright (no subprocess spawned) whenever the slot is occupied by anything other than the exact model already owned — replacing an owned model, or evicting an attached one, always requires an explicit prior `unload_model` call the user initiates, never an implicit swap.
- A model Lattice loads (`lms load <model_key> --identifier lattice-managed -y`, fixed argv, no shell) is confirmed exclusively through a follow-up `lms ps --json` probe reporting that exact identifier and model key — never through the load command's own exit code or stdout, which carry no documented meaning at all for this command.
- `unload_model` re-probes first and issues `lms unload <identifier>` only when the current probe still matches the persisted `Owned` record; any mismatch, `Attached`, or `Unknown` ownership is refused rather than attempted — the model-layer equivalent of "no kill-by-name or attached-resource stop."
- Every load/unload operation is bounded and cancellable, reusing 0.6's generalized `run_bounded_command` deadline/cancellation-flag pattern with an independent `model_operation_cancelled` flag so a model operation and a runtime operation can never cancel each other. Cancelling or timing out never issues a corrective unload for a load that may have partially succeeded server-side: the next probe is authoritative, and a model it later shows loaded anyway is recorded `Attached`, not `Owned`, since the operation that requested it already received its terminal outcome and does not get to retroactively claim ownership.
- No new dependency was added, for the same reason ADR 0010 needed none: `std::process`/`std::thread`/`std::sync::atomic`, generalized once already in 0.6, cover fixed-argv spawn, bounded polling and cancellation for this new resource type too. `serde_json::from_str` into narrowly-typed, `#[serde(default)]`-tolerant structs gives the same fail-closed behavior (unparsable or unexpectedly-shaped output becomes an empty inventory or `Unknown`/`Failed`, never a panic) that 0.5 already established for equally-undocumented daemon/server JSON.
- Load ownership survives a Lattice restart on exact identifier and model-key match alone, accepting the same residual risk ADR 0010 already documents for PID/identity reuse — not a new risk this change introduces.

Full behavioral detail lives in `openspec/changes/local-models/design.md`; this ADR records the decision, not the implementation.

## Consequences

- Lattice can load and unload exactly one model it provably owns on an already-running, already-approved runtime, without ever evicting a model an external session loaded or attaching a second managed slot.
- Success/failure of `load`/`unload` is always decided by a fresh, independent `ps --json` probe, never by either command's own exit code or output — a stricter requirement than ADR 0010's daemon case, because unlike `daemon up` there is no documented JSON shape to even optimistically attempt to parse from the mutating call itself.
- The exact JSON field names `lms ls --json`/`lms ps --json` emit remain unverified in this environment; the parser is deliberately tolerant (fail-closed to an empty inventory or `Unknown` rather than a panic) but may need alias/field adjustments once a real sample is captured on a machine where the daemon can wake. This is tracked as pending real-CLI evidence, not closed by this change.
- A stored `Owned` record surviving restart is accepted only on exact identifier and model-key match; PID/identity-reuse-adjacent residual risk is accepted as in ADR 0010, not newly introduced here.
