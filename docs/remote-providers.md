# Remote Providers

Status: implemented in 0.11 on the active roadmap branch. This page covers how remote OpenAI-compatible providers are configured, what leaves the machine, and what has actually been tested. Decisions are recorded in [ADR 0014](adr/0014-remote-provider-egress.md); behavior is specified in `openspec/changes/remote-openai-compatible-chat`.

## Setup

1. **Settings → Providers & credentials → Credentials:** add a credential. Lattice opens a native macOS prompt for the API key; the key goes to the Keychain and is never shown in the app.
2. **Settings → Remote providers:** add a provider with a name, its HTTPS base URL (for OpenAI, `https://api.openai.com/v1`), the model name to use, and optionally that credential.
3. **Chat:** pick the provider in the **Provider** menu. The first time you send, Lattice shows what will be sent and where. Choose **Allow and send** to approve that endpoint, or **Cancel** to send nothing.

To stop sending to a provider, use **Revoke approval** or remove the provider. Removing a provider keeps its credential and your conversations.

## Egress policy

- **Nothing is sent without a chat request.** Adding, editing, selecting or approving a provider never contacts it.
- **Consent is per destination.** Approval is stored for the exact normalized endpoint you saw. It stays until you revoke it or change the endpoint.
- **Changing the endpoint resets trust.** Saving a different endpoint removes the credential binding and the approval; you must choose the credential and approve again.
- **What is sent:** the visible conversation for that request, including earlier replies from other providers, the model name and an output limit. Nothing else: no workspace files, settings, credential labels or other conversations.
- **Where it goes:** only the configured endpoint over HTTPS with certificate verification against bundled Mozilla roots. Redirects are refused, not followed. Proxy environment variables are ignored.
- **The key:** read from the Keychain only for that request, sent only in the `Authorization: Bearer` header, never stored in SQLite, logs, IPC responses, error messages or URLs, and not cached afterwards.
- **Failures:** reported as `provider.auth_failed` (401/403), `provider.rate_limited` (429), `provider.timeout`, `provider.unavailable` (network, TLS, 5xx) or `provider.rejected` (redirects, other 4xx, error payloads inside a stream). The provider's error text is not shown. Lattice never retries automatically.
- **Cancel:** Lattice stops showing the reply right away. It closes the connection when the next chunk arrives, or at the 5-minute deadline if none arrives. The provider may bill for tokens generated before that.

History keeps which provider produced each reply as `local-openai-compatible` or `remote-openai-compatible` plus the model name. It never stores the endpoint or the credential.

## Pinned protocol subset

| Aspect         | Remote destination                                                                 |
| -------------- | ---------------------------------------------------------------------------------- |
| Request        | `POST {endpoint}/chat/completions`, `Content-Type: application/json`               |
| Authentication | `Authorization: Bearer <key>` when a credential is bound; otherwise no header      |
| Body           | `model`, `messages` (`system`/`user`/`assistant` with string `content`), `stream: true`, `max_completion_tokens` |
| Omitted turns  | Assistant messages with empty text (for example a reply that failed before its first token) |
| Not sent       | tools, `stream_options`, `n`, sampling parameters, images or other content parts  |
| Stream         | Server-Sent Events; `data:` lines with `choices[0].delta.content` or `delta.refusal`, ended by a `finish_reason` or `data: [DONE]` |
| Finish reasons | `length` → output limit reached; `stop`, `content_filter` and any other value → stop |
| Limits         | 32,000 prompt characters, 1,024 output tokens, 1 MiB per stream line, 15 s connect, 5 min per request |

The local llmster destination uses the same parser with `…/v1/chat/completions`, `max_tokens` and no authentication.

## Conformance matrix

| Destination                              | Evidence                                                                                          | Status |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------- | ------ |
| Loopback fixtures (local + remote shapes) | Rust tests: request shape, bearer header, status mapping, redirects, stream errors, oversized lines, deadline, cancellation, shared history | Passing |
| `https://api.openai.com/v1`, no credential | Opt-in real test `real_remote_unauthenticated_https_request_is_auth_failed`: real TLS handshake, `401` → `provider.auth_failed` | Passing (2026-09-16) |
| `https://api.openai.com/v1`, real key, streaming | Opt-in real test `real_remote_authenticated_stream_completes`                                      | Pending: no maintainer credential in the verification environment |
| Other OpenAI-compatible services (OpenRouter, Groq, Together, Mistral, DeepSeek, self-hosted gateways) | None | Untested; may reject `max_completion_tokens` or differ in stream details |

Run the opt-in tests with:

```bash
LATTICE_REMOTE_SMOKE=1 cargo test -p lattice-core real_remote_unauthenticated -- --ignored

LATTICE_REMOTE_SMOKE=1 \
LATTICE_REMOTE_SMOKE_ENDPOINT=https://api.openai.com/v1 \
LATTICE_REMOTE_SMOKE_MODEL=<model> \
LATTICE_REMOTE_SMOKE_API_KEY=<key> \
cargo test -p lattice-core real_remote_authenticated -- --ignored
```

The authenticated test sends one short prompt with a 64-token output limit. Keep the key out of shell history, and never add it to CI for untrusted pull requests.

## Known limitations

- HTTPS only. Local servers other than the managed llmster runtime, and plain-HTTP endpoints, are not supported.
- Only the bundled Mozilla roots are trusted. Enterprise roots and TLS-inspecting proxies do not work.
- The Chat provider selection resets to **Local model** on restart.
- Approval and credential binding happen in the app window, not through a native confirmation (see ADR 0014).
