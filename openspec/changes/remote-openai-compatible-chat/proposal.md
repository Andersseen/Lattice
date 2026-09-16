# Remote OpenAI-Compatible Chat

## Objective

Let the user continue the same canonical conversation through a remote OpenAI-compatible HTTPS provider — selected per request, authenticated with a 0.10 credential reference, and gated by an explicit, destination-bound consent — and switch back to local chat without losing history, identity or provenance.

## Rationale

The roadmap's critical path puts "credentials precede remote requests" and "Provider abstraction starts with the first real completion consumer, avoiding a later local-chat rewrite" (`docs/roadmap.md`). 0.8 (`local-streaming-chat`) defined the canonical `ChatRequest`/`ChatStreamEvent` port and explicitly deferred a wider provider boundary to "0.11 is the point where a real second implementation earns that abstraction". 0.9 (`conversation-persistence`) recorded `provider_key`/`model_key` per assistant message "so 0.11+ remote providers do not need a schema migration". 0.10 (`os-secure-credentials`) added `CredentialStore::resolve_secret` as the only path to a usable secret and named 0.11 its first Rust-internal consumer. This change is that consumer and that substitution.

Exploration findings recorded before any code (2026-09-16):

- **Canonical port holds without redesign.** Remote chat needs no change to `ChatRequest`, `ChatMessage`, `ChatStreamEvent` or `ChatFinishReason`. Only `StartChatStreamRequest` — already a command-level wrapper introduced by 0.9 exactly so persistence could amend it without touching the port — gains a `target` selecting local or a remote profile.
- **One protocol family, two destinations.** A remote OpenAI-compatible provider speaks the same Server-Sent Events chat-completions protocol 0.8's `local_openai.rs` already parses. Copying the parser would create two drifting implementations of one wire format; this change turns the local adapter into one `openai_compatible` adapter configured per destination (local loopback vs. remote HTTPS), which is the smallest honest "second implementation" of the 0.8 port. No provider trait is introduced: 0.12 (Anthropic) is the first genuinely different protocol and earns it.
- **Pinned protocol subset**, checked against OpenAI's current published OpenAPI document (`openapi.documented.yml`, `info.version: 2.3.0`, server `https://api.openai.com/v1`, fetched 2026-09-16): `POST {base}/chat/completions`, `Authorization: Bearer`, string `content` for `system`/`user`/`assistant`, `stream: true` with `choices[].delta.content` chunks, `finish_reason` in `stop`/`length`/`tool_calls`/`content_filter`/`function_call`, terminated by `data: [DONE]`. `max_tokens` is documented as "now deprecated in favor of `max_completion_tokens`" and incompatible with reasoning models, so the remote destination sends `max_completion_tokens` while the local llmster destination keeps LM Studio's documented `max_tokens`. OpenAI's error guide maps 401 to invalid authentication, 403 to unsupported region, 429 to rate/credit limits, 500/503 to server errors/overload — the normalized error codes below follow that table.
- **TLS is a real new dependency.** 0.8 deliberately built `ureq` with `default-features = false` and zero TLS crates, since only loopback HTTP existed. Measured on this workspace by toggling `ureq` features and diffing `Cargo.lock` (then reverting): `rustls` (ring + bundled `webpki-roots`) adds 8 crates (`ring`, `rustls`, `rustls-pki-types`, `rustls-webpki`, `subtle`, `untrusted`, `webpki-roots`, `zeroize`); `rustls` + `platform-verifier` adds 18 (including Android `jni` crates in the lockfile); `native-tls-no-default` adds 12 and pulls `openssl`/`openssl-sys` into the lockfile for Linux targets, which would make the 0.14 Linux/Windows preview builds depend on system OpenSSL headers. See design.md "Dependency admission".
- **Redirect and proxy defaults are unsafe for a credentialed request.** Read directly in `ureq` 3.4.1's `config.rs`: `max_redirects` defaults to 10 and `proxy` defaults to `Proxy::try_from_env()` (`ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY`). Both are set explicitly (0 redirects — the 3xx response is returned, not followed — and no proxy) so the only destination a request can reach is the exact configured endpoint. `ureq`'s own debug logging already redacts every header outside a fixed non-sensitive allowlist (`util.rs` `DebugHeaders`), so `Authorization` never reaches a log line even at debug level.
- **This environment can prove TLS/normalization but not an authenticated remote chat.** Network egress works here (crates index and OpenAI's public OpenAPI document were fetched), but no remote provider account or API key exists on this machine. An authenticated streaming smoke is therefore recorded as pending maintainer evidence, never fabricated; an opt-in unauthenticated HTTPS request is used to show real TLS + `401 → provider.auth_failed` normalization.

## Dependencies

- 0.8 local streaming chat: the canonical port, the reader/orchestrator cancellation design and the single active run.
- 0.9 conversation persistence: canonical history, per-message provider/model provenance, "no conversation is written for a request that fails its preconditions".
- 0.10 OS-secure credentials: `CredentialStore::resolve_secret`, availability states, no secret over IPC.
- 0.3 desktop security baseline and 0.4 storage: deny-by-default command permissions, versioned migrations (this change adds schema version 7).

## Scope

- `ProviderProfile` (label, HTTPS endpoint, remote model key, optional credential reference, destination-bound consent, revision) with list/create/update/delete, explicit credential binding and consent grant/revoke commands; a sibling `ProviderProfileStore` (schema version 7).
- Endpoint validation: `https://` only, no userinfo/query/fragment, bounded length; changing the endpoint clears both the credential binding and the consent.
- `StartChatStreamRequest.target` (`local` | `remote { profileId }`), resolved and authorized in Rust per request: model must match the profile, consent must be valid for the profile's current endpoint, the bound credential is resolved only at dispatch and never cached.
- One `openai_compatible` adapter serving the local (loopback HTTP, `max_tokens`, no auth) and remote (HTTPS, bearer auth, `max_completion_tokens`) destinations: no redirects, no proxy, bounded SSE line length, mid-stream `error` payloads fail the run, best-effort connection close on cancellation, normalized `provider.*` failure codes.
- Remote runs do not hold the local model lease; local unload is only refused while a local run is active.
- Assistant messages record `remote-openai-compatible` provenance plus the remote model key.
- Angular: a remote-provider section in Settings, a provider selector and consent disclosure dialog in Chat, provenance on assistant messages, browser fallbacks and focused tests.
- ADR 0014, a remote-provider egress/conformance document, and the 0.11 verification record.

## Non-goals

- Anthropic/Gemini adapters (0.12/0.13), a provider trait or registry, tool calls (0.15).
- Ollama/llama.cpp/vLLM lifecycle adapters, plain-HTTP or loopback remote profiles, provider marketplace/discovery, remote model listing.
- Automatic failover, automatic retry (including after `429`), background or selection-triggered requests.
- Sampling/temperature UI, `stream_options`/usage reporting, multimodal content, the OpenAI Responses API or any other API surface.
- Persisting the Chat provider selection across restart; persisting finish reasons; a native (non-WebView) consent dialog.
- OS/enterprise trust-store integration or TLS-intercepting proxies (bundled Mozilla roots only in 0.11).

## Impacted Capabilities

- `providers` (remote profile, egress, consent, adapter and error deltas)
- `chat-streaming` (per-request target selection, lease scope, best-effort remote cancellation)
- `application-api` (new checked commands)
- `conversations` (provider/model provenance that survives provider switching)

0.9 already stores `provider_key`/`model_key` per assistant message but its spec delta states no requirement about them; this change adds that requirement now that a second provider key value exists, without a schema change to `messages`. `local-storage`'s generic migration/backup requirements already cover schema version 7. `credentials`' "Rust-internal only" resolution requirement is consumed, not changed.
