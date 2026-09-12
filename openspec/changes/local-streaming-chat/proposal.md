# Local Streaming Chat

## Objective

Stream a text chat response from the one Lattice-managed loaded local model, allow the user to cancel it mid-stream, and do both through a provider-neutral completion port that a remote provider (0.11+) can later implement without a local-chat rewrite.

## Rationale

0.7 can load and inspect exactly one owned model on an already-running runtime, but nothing yet sends that model a request. The architecture sequence deliberately introduces the provider port with this first local streaming consumer rather than with an unused trait, so the shape proven here is the one 0.11 validates against a remote endpoint. llmster (LM Studio) exposes an OpenAI-compatible `/v1/chat/completions` endpoint on the already-discovered loopback port; `stream: true` switches it to Server-Sent Events, one `data: {...}` line per delta, terminated by `data: [DONE]` (checked 2026-09-12 against current `lmstudio.ai/docs/developer/openai-compat/chat-completions` and `.../openai-compat`).

This is also the first Lattice feature that needs a real HTTP client: 0.5/0.6's loopback "health" checks are a bare `TcpStream::connect`, not an HTTP request, and there is no async runtime anywhere in the workspace. Reusing that non-goal (no new dependency) here would mean hand-rolling HTTP/1.1 request framing plus chunked/SSE body parsing from scratch on a boundary that streams model-generated text into the UI — a materially higher defect surface than 0.5–0.7's fixed, fully-controlled subprocess argv. See design.md's "Dependency admission" for the considered alternatives and the concrete choice.

## Dependencies

- 0.7 installed model management (`openspec/changes/local-models/`, ADR 0011): `ModelSlotStatus`, `ModelLoadOwnership`, the model-key/identifier vocabulary this change leases against. Implemented on the active branch; real-catalog and P2 evidence remain pending per `docs/verification/0.7-installed-model-management.md` — this change proceeds against the accepted design/spec deltas of that still-open change, per the roadmap's explicit 0.8 handoff, the same posture 0.7 itself took toward 0.6.
- 0.6 llmster lifecycle (ADR 0010): the approved executable and loopback endpoint this change's HTTP client targets; the bounded-command/independent-confirmation pattern this change's cancellation reuses.
- 0.5 ModelRuntime discovery: `ModelRuntimeStatus.endpoint`, the source of the base URL for the completions request.

## Scope

- One canonical text completion port (`ChatMessage`, `ChatRequest`, `ChatStreamEvent`) with no llmster-specific field anywhere in its public shape.
- One Rust OpenAI-compatible local transport adapter translating the canonical request into an LM Studio chat-completions HTTP request and its SSE deltas back into canonical stream events.
- One Tauri streaming command (`start_chat_stream`) taking a `tauri::ipc::Channel` and returning immediately with a run identity, plus `cancel_chat_stream`; a background thread performs the blocking HTTP call and pushes events, mirroring 0.6/0.7's existing thread-plus-shared-`AtomicBool` cancellation pattern instead of introducing an async runtime.
- Exactly one active run globally (the same product limit the architecture doc already states for run + loaded model), enforced by new in-process `DesktopState` fields; a chat run in flight refuses a concurrent `unload_model` rather than racing it.
- A minimal in-memory Chat page: message list, plain-text streaming render, send, cancel. No conversation list, no persistence — every reload starts empty.
- Explicit, enforced context/output/deadline limits and a bounded per-run byte cap, per the Definition of v1's requirement that 0.8 turn its stated targets into real enforced numbers.
- Completion of the P2 profile's active-stream measurement (cold vs. warm generation, memory during streaming) the Definition of v1 already assigns to 0.8, for each qualified candidate profile from 0.7.

## Non-goals

- Conversation persistence, history, or reopening (0.9).
- Any remote provider, credential, or network egress beyond the already-approved local loopback endpoint (0.10/0.11+).
- Tool calls, tool-result blocks, or any agent loop (0.15).
- Markdown/rich rendering, syntax highlighting, or token-level UI affordances beyond plain incremental text.
- Sampling-parameter UI (temperature, top-p, etc.); the adapter picks fixed internal defaults.
- Automatic retry, fallback, or reconnect-and-replay of a generation after disconnect or app restart.
- More than one simultaneously active run, or a second Lattice-managed loaded model.

## Impacted Capabilities

- `chat-streaming` (new)
- `providers` (new)
- `application-api`

`local-models` (0.7) is a dependency this change reads (model slot ownership) but does not modify: `unload_model`'s refusal while a chat run is active is expressed as a `chat-streaming` requirement guarding the shared resource, not a delta against 0.7's still-open spec, matching 0.7's own choice not to modify 0.6's `model-runtime` delta while it was still open.
