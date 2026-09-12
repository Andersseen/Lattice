# Local Streaming Chat Design

## Ownership

New domain area, not an extension of `model_runtime`: `ModelRuntime` manages discovery/lifecycle/model-slot state (0.5–0.7); `Provider` handles inference requests and streaming, exactly as the architecture sequence's responsibility map separates them. `lattice-core` keeps owning canonical request/event shapes, run identity, limits and the lease check against the model slot. `lattice-desktop` exposes the streaming command/channel and holds the shared cancellation flag and single-active-run guard, exactly as it already does for runtime/model operations. Angular owns only Chat page presentation and calls the typed application API.

New module, sibling to `model_runtime`:

- `crates/lattice-core/src/providers/mod.rs` — re-exports.
- `crates/lattice-core/src/providers/completion.rs` — canonical `ChatMessage`, `ChatRole`, `ChatRequest`, `ChatStreamEvent`, `ChatFinishReason`, run identity/sequence bookkeeping, limit constants, and `start_chat_stream`/orchestration that checks the model lease (via `model_runtime::models`) before calling the adapter. No llmster-shaped field lives here.
- `crates/lattice-core/src/providers/local_openai.rs` — the isolated vendor adapter: builds the LM Studio chat-completions JSON body, opens the HTTP request, parses SSE lines, and translates each parsed delta/finish/error into a canonical `ChatStreamEvent`. This is the only file allowed to know the wire shape of an OpenAI-compatible request/response.

No new crate. `providers` stays inside `lattice-core`, same as `model_runtime`; there is exactly one real consumer (the local adapter) and no second provider exists yet to justify a trait boundary wider than one function signature — 0.11 is the point where a real second implementation earns that abstraction, not this change.

## Contracts

```rust
pub const START_CHAT_STREAM_COMMAND: &str = "start_chat_stream";
pub const CANCEL_CHAT_STREAM_COMMAND: &str = "cancel_chat_stream";

#[serde(rename_all = "camelCase")]
pub enum ChatRole { System, User, Assistant }

pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String, // bounded, see "Limits"
}

pub struct ChatRequest {
    pub model_key: String,   // must equal the currently Owned model's key; anything else is refused
    pub messages: Vec<ChatMessage>,
}

pub struct ChatRunHandle {
    pub run_id: String, // uuid v4, reusing the already-resolved `uuid` crate (see "Dependency admission")
}

pub struct CancelChatStreamRequest {
    pub run_id: String, // must match the currently active run; otherwise a no-op response, never an error
}

pub enum ChatFinishReason { Stop, MaxOutputTokens }

#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ChatStreamEvent {
    Started { run_id: String, model_key: String },
    Delta { run_id: String, sequence: u64, text: String },
    Completed { run_id: String, sequence: u64, finish_reason: ChatFinishReason },
    Cancelled { run_id: String, sequence: u64 },
    Failed { run_id: String, sequence: u64, error: AppError },
}
```

`start_chat_stream(request: ChatRequest, channel: tauri::ipc::Channel<ChatStreamEvent>, state) -> Result<ChatRunHandle, AppError>` is a plain synchronous `#[tauri::command] fn`, matching every existing command — no async runtime is introduced anywhere in the workspace by this change. It performs one fast, non-mutating precondition check (current model slot ownership, read through the existing settings-store lock exactly like `load_model`/`unload_model` already do), assigns a run ID, spawns one `std::thread` that owns the HTTP call and pushes events onto `channel`, and returns the handle immediately — before the thread produces its first byte, so the frontend always has the channel bound before any event can arrive. `cancel_chat_stream` only ever sets a shared flag and returns; it never blocks on the streaming thread.

`ChatMessage.role`/`text` and `ChatRequest.model_key` carry no llmster field (no `logprobs`, no vendor sampling knob); `local_openai.rs` fills in the adapter's own fixed internal sampling defaults when building the wire request, per the non-goal on sampling-parameter UI.

## Model lease and run ownership

`DesktopState` gains:

```rust
active_chat_run: Mutex<Option<ActiveChatRun>>, // ActiveChatRun { run_id: String, cancel: Arc<AtomicBool> }
```

- `start_chat_stream` locks `active_chat_run`; if it is already `Some`, it returns `AppError::chat_conflict` ("A response is already streaming.") without reading the model slot or spawning anything — one run globally, matching the architecture doc's stated product limit.
- It then reads the current `ModelSlotStatus` through the existing settings-store lock (no new fresh `lms ps --json` probe is spawned here; 0.7 already keeps this reconciled on every settings-store read). If `ownership` is not `Owned` with a `model_key` equal to `request.model_key`, it returns `AppError::chat_invalid` ("Load the requested model before starting a chat.") — a chat run never triggers an implicit load, exactly as 0.7 never triggers an implicit load from a mismatched request.
- Only once both checks pass does it write `Some(ActiveChatRun { run_id, cancel })` and spawn the worker thread. The lock is held only for the two checks and the write, never for the duration of the stream.
- The worker thread clears `active_chat_run` back to `None` on every terminal path (`Completed`, `Cancelled`, `Failed`, or a thread panic caught via a guard) before returning, so a crashed or errored run always frees the slot for the next request — covering the roadmap's "crash/error frees the active slot" acceptance criterion.
- `unload_model` (0.7) gains one precondition check in its Tauri command wrapper (`apps/desktop/src-tauri/src/lib.rs`), before it touches the settings store: if `active_chat_run` is `Some`, return `AppError::runtime_conflict` ("Cancel the active chat before unloading its model.") — reusing the existing `runtime.conflict` code rather than adding a near-duplicate, since the shape of the conflict (a live consumer holds the resource) is the same one 0.6 already codes that way. This is the one cross-layer touch this change makes into already-shipped 0.7 code. It is recorded as a `chat-streaming` requirement (`specs/chat-streaming/spec.md`, "Lattice SHALL Prevent The Managed Model From Being Unloaded While A Chat Run Holds Its Lease"), not a delta against 0.7's own `local-models` spec: `local-models` is still an open, unarchived change, and OpenSpec's own validator refuses a `MODIFIED` delta against a capability with no existing permanent spec (confirmed 2026-09-12 via `pnpm run openspec:validate`-equivalent `validate local-streaming-chat --strict`). This also matches 0.7's own choice not to add a delta against 0.6's still-open `model-runtime` spec while building on it.

## Stream lifecycle, limits and cancellation

The worker thread, once dispatched:

1. Builds the LM Studio request body: `{"model": model_key, "messages": [...], "stream": true, "max_tokens": MAX_OUTPUT_TOKENS}` and POSTs it to `format!("{endpoint}/v1/chat/completions")` using the already-approved endpoint from `ModelRuntimeStatus`.
2. Reads the response body line-by-line as Server-Sent Events. Each non-empty `data: ` line other than `data: [DONE]` is parsed as one JSON delta object; its `choices[0].delta.content`, when present, becomes one `ChatStreamEvent::Delta` with the run's next monotonically increasing `sequence` (starting at 0 for `Started`). A line that fails to parse as JSON, or whose shape does not match the expected delta object, ends the stream with `Failed` (`chat.failed`, "The local model runtime sent an unreadable response.") rather than silently skipping it — no llmster/model output is trusted enough to swallow a shape mismatch quietly.
3. Between each read, the thread checks `cancel`. If set, it stops reading, does not wait for `[DONE]`, and emits exactly one `Cancelled` event as the terminal event — never also `Completed`.
4. Natural end of stream (`data: [DONE]` observed, or `choices[0].finish_reason` present) emits exactly one `Completed` event carrying `ChatFinishReason`.
5. A connection error, a non-2xx HTTP status, or the deadline below being reached emits exactly one `Failed` event. Every terminal branch is mutually exclusive by construction (the read loop has one `break` per branch, followed immediately by exactly one terminal `channel.send`), which is what gives "exactly one terminal outcome per run" its guarantee — not a runtime assertion, a structural one.

Limits (Definition of v1 requires 0.8 to turn its stated targets into real enforced numbers, not carry them forward as prose):

- **`MAX_OUTPUT_TOKENS = 1024`** sent as the request's own `max_tokens`, capping generation length at the source rather than truncating a longer response after the fact. Provisional: no qualified candidate profile's real context window has been read from a live catalog yet (0.7's own P2 task is still pending for the same environment reason below), so this is a conservative default sized well under both candidate profiles' expected context windows, not a measured figure. Revisit once 0.7's real `lms ls --json`/`--detailed` sample is captured.
- **`STREAM_DEADLINE = Duration::from_secs(300)`** (5 minutes) wall-clock from `Started`, matching the Definition of v1's existing 5-minute run-deadline default rather than inventing a second number; reaching it emits `Failed` (`chat.failed`, "Generation took too long and was stopped.").
- **`IDLE_READ_TIMEOUT = Duration::from_secs(30)`**: the underlying socket read is bounded so a server that stops sending bytes without closing the connection cannot block the worker thread forever; exceeding it is the same `Failed` outcome as the overall deadline, not a distinct code, since the user-observable effect is identical. Confirming the exact `ureq` API surface for a per-read (not total-request) timeout is a pinned implementation task (see tasks.md); if the installed version cannot express an idle-only timeout, the fallback is a lower total-request timeout plus this deadline, documented as a narrower guarantee at that point rather than silently assumed.
- **Prompt bound**: the sum of `messages[].text` is bounded to `MAX_PROMPT_CHARS = 32_000` characters (a conservative byte-based proxy given no verified per-model token/context figure exists yet, per the `MAX_OUTPUT_TOKENS` note above); a request over that bound is refused before any HTTP call with `AppError::chat_invalid` ("The conversation is too long for this model."), not truncated silently.
- **Bounded event delivery, no buffer growth**: each parsed SSE line is forwarded as one `channel.send` immediately; the worker never accumulates unsent deltas in an application-level queue. Backpressure is therefore whatever `tauri::ipc::Channel`/the OS socket already provide; no additional bounded queue is introduced because there is no evidence yet of unbounded growth to fix (Definition of v1: "a failed measurement blocks the gate ... it cannot be converted into a performance claim" applies here in reverse — no measurement means no invented mitigation either). If soak testing later shows otherwise, that is a P4/0.25 finding, not assumed here.

## Shutdown

`stop_owned_model_runtime_for_shutdown` (0.6) already stops the owned daemon/server on exit. This change adds one step before that: on `RunEvent::Exit`, if `active_chat_run` is `Some`, set its `cancel` flag (best-effort; the process is exiting regardless) so the worker thread's next read-loop check, if it still gets one, does not attempt a corrective action. No new blocking wait is introduced into shutdown; an in-flight HTTP connection is simply dropped when the process exits, matching how 0.6/0.7 already treat an interrupted external side effect as unknown rather than something shutdown must resolve.

## Storage

None. This is a deliberate non-goal (0.9 owns persistence); no settings schema migration, no new SQLite table. `active_chat_run` and per-run sequence counters are process-memory only and do not survive restart — a restart simply has no active run, which is indistinguishable from the normal "nothing streaming" state and needs no reconciliation step (unlike 0.7's load ownership, there is no external resource to reconcile against, since llmster has no notion of a Lattice "run").

## UI

New `apps/desktop/src/app/pages/chat/` (`chat.page.ts`/`.html`/`.css`), following the existing single-page-per-feature shape (`pages/models/`). New `core/api/chat-fallback.ts` (browser fallback: an explicit "requires the desktop app" unavailable state, matching `model-slot-fallback.ts`'s pattern) and `core/state/chat.store.ts` holding the in-memory message list, current run ID, and streaming/idle/error UI state. The page renders plain text only (no markdown), appends `Delta` text to the last assistant message, and shows a Cancel action while a run is active, disabled once a terminal event lands. No new route-level persistence; navigating away and back within the same session keeps the in-memory store (no reload-survival claim, consistent with "Storage: none" above).

## Dependency admission

**HTTP client.** Considered:

- **No new dependency (hand-rolled HTTP/1.1 over `std::net::TcpStream`).** Rejected: unlike every prior 0.5–0.7 subprocess/TCP-connect probe, this requires correctly framing an HTTP POST, parsing response headers, and handling chunked transfer-encoding before SSE parsing can even start — real protocol-parsing surface `std` does not provide, on a path that streams model text directly into the UI. The project's own precedent (0.5/0.6/0.7 ADRs) rejected new dependencies only where `std` already covered the actual problem (process spawn, polling, JSON deserialization); here it does not.
- **`reqwest` + `tokio`.** Rejected: would introduce the workspace's first async runtime for exactly one synchronous, thread-per-request loopback call, adding a second concurrency model alongside the `std::thread`/`AtomicBool` pattern already used everywhere else, plus meaningfully larger compile time and binary size, to solve a problem that does not need concurrency beyond "don't block the Tauri command."
- **`ureq`, `default-features = false`.** Accepted. Sync/blocking API matching the existing thread-per-operation style exactly; dual MIT/Apache-2.0 licensed; latest release `3.4.0` (2026-08-08, confirmed via docs.rs listing 2026-09-12). Default features pull in `rustls`+`gzip`; disabling default features drops both, since the only target is plain `http://127.0.0.1:<port>` to an already-approved loopback endpoint with no compression negotiation needed — keeping the added dependency tree to the HTTP/1.1 client itself. Exact feature-flag combination (whether any minimal feature must stay enabled for plain-HTTP POST + streaming body reads) is confirmed against the pinned version during implementation, not assumed here; if `default-features = false` alone does not build, the narrowest feature set that does is used and recorded in tasks.md, not a silent fallback to defaults.

**Run identity.** `uuid` (`v4`, `default-features = false`) is added as a direct `lattice-core` dependency. This is not a new entry in the compiled dependency tree: `uuid` `1.26.0` is already resolved transitively (confirmed in `Cargo.lock`, pulled in by the existing Tauri/Tao/Wry stack), so this only formalizes reliance on a crate already present in every build rather than hand-rolling a timestamp/counter identifier scheme.

No process-management or JSON-schema crate is needed for the same reasons 0.5–0.7 already established: `serde_json` deserialization into narrow, tolerant structs covers the SSE delta shape (a documented one, unlike `lms ls`/`ps`, since this is OpenAI's own public streaming format), and the existing `std::thread`/`AtomicBool` pattern covers cancellation.

## Documentation sources

Checked 2026-09-12:

- `https://lmstudio.ai/docs/developer/openai-compat/chat-completions` and `https://lmstudio.ai/docs/developer/openai-compat` — `/v1/chat/completions` request/response shape is documented as identical to OpenAI's own spec, including `messages`, `stream: true`, and standard sampling parameters; streaming responses are Server-Sent Events, one `data: {...}` JSON delta per line, terminated by `data: [DONE]`.
- `https://github.com/algesten/ureq` — dual MIT/Apache-2.0 license; default features are `rustls` + `gzip`; supports incremental/streaming response body reads (`Body::as_reader()`) over a blocking API; a global request timeout is configurable on the agent. The exact API for an idle-only (per-read) timeout, as opposed to a total-request timeout, is confirmed against the pinned version during implementation (see "Stream lifecycle, limits and cancellation" and tasks.md), not asserted here from documentation alone.

Not yet checked in this environment: a live SSE sample from this machine's own llmster installation. The same headless-daemon-wake gap 0.6/0.7 already recorded (`docs/verification/0.6-llmster-lifecycle.md`, `docs/verification/0.7-installed-model-management.md`) blocks a real end-to-end capture here; the SSE framing above is taken from LM Studio's own current public documentation, not independently re-verified against this installation. Capturing one real sample is a blocking implementation task, not assumed correct.

## Verification

Testing tier follows the Definition of v1's 0.5–0.8 row: disposable loopback fixture HTTP server for streaming/cancel tests (fragmented SSE lines split across TCP reads, malformed delta JSON, mid-stream disconnect, a fixture that sends two terminal-shaped payloads to prove the second is never forwarded, cancel-before-first-token and cancel-mid-stream), plus an opt-in real llmster smoke test against each 0.7 qualified candidate profile, recorded as skipped (never passed) when the prerequisite runtime cannot be woken in the running environment.

P2 profile addition (`docs/verification/0.8-local-streaming-chat.md`, produced during the Verify step, not part of this design): active-stream memory/CPU during generation for each qualified candidate, completing the P2 gate the Definition of v1 explicitly defers to 0.8 ("P2's active-stream measurement is completed in 0.8, not faked in 0.7").
