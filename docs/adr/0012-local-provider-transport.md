# 0012 - Local Provider Transport

## Status

Accepted

## Context

0.8 is the first Lattice feature that needs a real HTTP client: 0.5/0.6's loopback "health" checks are a bare `TcpStream::connect`, not an HTTP request, and no async runtime exists anywhere in the workspace. ADR 0005 assigns `Provider` (inference/streaming) a boundary separate from `ModelRuntime` (discovery/lifecycle/model slot), introduced with its first real consumer rather than an unused trait; this change is that first consumer, targeting llmster's OpenAI-compatible `/v1/chat/completions` endpoint (checked 2026-09-12 against current `lmstudio.ai/docs/developer/openai-compat` docs: `stream: true` switches to Server-Sent Events, one `data: {...}` JSON delta per line, terminated by `data: [DONE]`).

Reusing 0.5–0.7's "no new dependency" pattern here would mean hand-rolling HTTP/1.1 request framing and chunked-transfer-encoding body parsing from scratch on a boundary that streams model-generated text directly into the UI — real protocol-parsing surface `std` does not provide, unlike the fixed subprocess argv/JSON-deserialization problems those ADRs actually solved without one.

## Decision

Add `ureq` (`3.4.0`, resolved by Cargo to patch `3.4.1`) to `lattice-core` with `default-features = false`. Confirmed directly in the resulting `Cargo.lock` — not assumed from documentation — that this adds only `httparse`, `http`, `base64`, `ureq-proto`, `utf8-zero`, `libc`, and `log`; zero `rustls`/`native-tls` dependencies, since the only target is plain `http://127.0.0.1:<port>` to an already-approved loopback endpoint. The request body is serialized with the already-present `serde_json` and sent as raw bytes; `ureq`'s own optional `json` feature is not enabled.

Add `uuid` (`1.26.0`, `v4`, `default-features = false`) for chat run identity. This is not a new entry in the compiled dependency tree: `uuid` was already resolved transitively through the Tauri/Tao/Wry stack (confirmed in `Cargo.lock` before this change), so this only formalizes reliance on an already-present crate.

`crates/lattice-core/src/providers/` is a new module, sibling to `model_runtime/`, not a new crate: `completion.rs` owns the canonical `ChatMessage`/`ChatRequest`/`ChatStreamEvent` port, run identity/sequencing, and the model-lease precondition (`authorize_chat_request`, checked against `ModelSlotStatus` before any dispatch); `local_openai.rs` is the one module allowed to know llmster's OpenAI-compatible wire shape, translating it into canonical events. No llmster-specific field crosses into `completion.rs`'s public types.

`ureq`'s only body-read timeout (`timeout_recv_body`) is a **total** budget, confirmed by reading its 3.4.1 source directly, not an idle/per-read one — a blocking read only returns early once bytes actually arrive, so relying on it alone would make "cancel before the model's first token" only eventually true, not promptly true. `run_chat_stream` therefore spawns two threads per run: a reader thread that owns the blocking HTTP/SSE call, and an orchestrator thread that polls an `mpsc::Receiver` every `ORCHESTRATOR_POLL_INTERVAL` (200ms), checking the shared cancellation flag first on each tick. A cancelled reader thread is abandoned, not forcibly aborted — Rust has no safe thread-kill primitive and `ureq` exposes no cooperative-abort handle for a call already in flight — bounded at worst by `STREAM_DEADLINE` (`timeout_recv_body`, 5 minutes). The active-run slot itself (`DesktopState.active_chat_run` in the Tauri shell) is cleared by an RAII `Drop` guard constructed on the spawned thread before `run_chat_stream` runs, so a normal return _and_ an unexpected panic unwinding through that thread both free it — confirmed no `panic = "abort"` profile exists anywhere in the workspace, so Rust's default unwind strategy actually runs `Drop` on that path.

A disposable `std::net::TcpListener` fixture test (multi-write response, connection closed with zero delay after the last write) reproduced a real timing edge case in `ureq` 3.4.1's `CloseDelimited` body reader: the final chunk of a fragmented response can be lost if the connection closes immediately after the last write. Fixed in the test fixture with a trailing delay (matching realistic server timing, not hiding a bug in this adapter's own parsing, which was independently verified correct first); recorded as an open item to re-check against a real llmster response once this environment's headless-daemon-wake blocker (already tracked for 0.6/0.7) is resolved.

Full behavioral detail lives in `openspec/changes/local-streaming-chat/design.md`; this ADR records the decision, not the implementation.

## Consequences

- The workspace gains its first real HTTP client and its first multi-thread-per-request pattern, both confined to `lattice-core::providers`; no async runtime is introduced anywhere.
- Local completions traverse the same canonical `ChatRequest`/`ChatStreamEvent` port 0.11 must later satisfy for a remote provider — no llmster-specific field is visible outside `local_openai.rs`.
- Cancellation is responsive (bounded by `ORCHESTRATOR_POLL_INTERVAL`, not `STREAM_DEADLINE`) at the accepted cost of an abandoned, bounded reader thread per cancelled run — a residual cost of the same class ADR 0010/0011 already accept for a cancelled subprocess operation whose external effect cannot be forcibly undone.
- The exact SSE delta shape is taken from LM Studio's current public documentation, not yet independently verified against this installation's own output, for the same environment reason 0.6/0.7 record; a found `ureq` CloseDelimited timing edge case is documented, not silently worked around.
