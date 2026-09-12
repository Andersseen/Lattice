# Installed Model Management

## Objective

Report an honest installed-versus-loaded model inventory and manage exactly one Lattice-owned loaded model slot on the already-started, already-approved runtime, without disrupting a model an external session loaded.

## Rationale

0.6 can start and stop the llmster daemon/server but never inspects or manages what runs inside it. 0.8's streaming chat needs a known, loaded model to target, and the Definition of v1's P2 gate requires measured cold/warm load, unload and reload numbers per qualified profile. Directly invoking `lms load`/`lms unload` is riskier than it looks: unlike `daemon status`/`server status`, neither command documents or was observed to emit `--json` output (confirmed 2026-09-10 against both `lms load --help`/`lms unload --help` on the installed CLI, commit `71bd99c`, and the current `lmstudio.ai` CLI reference — see design.md's "Documentation sources"). A design that trusted either command's own exit code or stdout for success, identity, or failure reason would repeat the exact mistake 0.6/ADR-0010 already ruled out for `daemon up`; this change reuses that same ownership-and-independent-confirmation pattern for models instead of inventing a new one.

## Dependencies

- 0.6 llmster lifecycle (`openspec/changes/llmster-lifecycle/`, ADR 0010): owned/attached ownership vocabulary and the bounded-command/independent-probe pattern this change reuses. Implemented on the active branch; real-runtime start/stop evidence is recorded pending in `docs/verification/0.6-llmster-lifecycle.md`, not yet archived — this change proceeds against the accepted design/spec deltas of that still-open change, per the roadmap's explicit 0.7 handoff.
- 0.5 ModelRuntime discovery: approved executable, `probe_model_runtime`, loopback conventions.
- 0.4 persistent application settings: transactional migration path this change extends to schema v4.

## Scope

- Read the installed inventory (`lms ls --json`) and the live loaded inventory (`lms ps --json`) through one checked IPC status command, refreshed on demand like 0.5's discovery status.
- Load and unload exactly one Lattice-managed model identifier (`lms load <model-key> --identifier <fixed-identifier>` / `lms unload <identifier>`), with success, failure and identity always taken from a follow-up `lms ps --json` probe, never from the load/unload command's own exit code or stdout.
- Record `ModelLoadOwnership` (`owned`, `attached`, `unknown`) the same way `RuntimeOwnership` is recorded for the daemon: a model already loaded when Lattice first checks is `attached` and is never a candidate for Lattice-initiated unload.
- Refuse to load a second model while an owned model is already loaded; the user must explicitly release (unload) the idle owned model first. No implicit eviction of an owned or attached model.
- Bound and allow cancellation of an in-flight load/unload, mirroring 0.6's deadline/cancel-flag pattern.
- Persist load ownership across restart using the same fingerprint/identity reconciliation approach as runtime ownership, migrating settings to schema v4.
- Minimal Models UI additions: installed/loaded list, select-and-load, unload, pending/failure state.
- Select one small Qwen and one small Gemma candidate profile and record manual acquisition guidance (point at llmster's own `lms get` / LM Studio's model browser; Lattice never downloads a model itself). Exact model key, quantization and file size are recorded once verified against a real reachable runtime catalog (see tasks.md), not guessed from documentation.
- Produce the P2 profile: cold load, warm reload, and unload memory/time measurement for each qualified candidate, separating model increment from the already-measured (P1) idle runtime baseline.

## Non-goals

- Model downloader/marketplace UI; acquisition stays a documented manual step outside Lattice.
- Inference, streaming, cancellable generation, or any chat surface (0.8).
- Quantization/architecture internals beyond what is needed to display and select a model.
- Whole-model-family compatibility claims; only the two named candidate profiles are qualified.
- Automatic idle-based unload. The existing `idleUnloadMinutes` (0.4) setting stays a stored, unconsumed preference; only an explicit user action loads, unloads, or replaces the managed model.
- More than one simultaneously Lattice-managed loaded model slot.
- Any change to `daemon`/`server` lifecycle behavior itself; this change only adds a model layer on top of an already-running runtime.

## Impacted Capabilities

- `local-models` (new)
- `application-api`
- `local-storage`
