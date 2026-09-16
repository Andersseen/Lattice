# 0014 - Remote Provider Egress

## Status

Accepted

## Context

0.11 is the first time Lattice sends conversation context off the machine and the first consumer of a 0.10 credential. Until now the only HTTP client (`ureq`, ADR 0012) was built without TLS and only talked to an approved loopback llmster endpoint. A remote OpenAI-compatible provider needs HTTPS, a bearer credential, and rules that keep both the context and the key going only where the user said they may go.

Findings recorded before implementation (2026-09-16):

- `ureq` 3.4.1 defaults, read in its `config.rs`: `max_redirects = 10`, `proxy = Proxy::try_from_env()` (`ALL_PROXY`/`HTTPS_PROXY`/`HTTP_PROXY`), `http_status_as_error = true`. Following redirects or an environment proxy would let a request carrying `Authorization` reach a host other than the configured endpoint.
- `ureq`'s debug logging (`util.rs` `DebugHeaders`) prints only an allowlist of non-sensitive headers and redacts the rest, so `Authorization` is never logged by the dependency.
- TLS options, measured by toggling features and diffing `Cargo.lock` on this workspace: `rustls` (ring + `webpki-roots`) adds 8 crates plus a second `windows-sys` version that `ring` only uses on Windows ARM64; `rustls` + `platform-verifier` adds 18; `native-tls-no-default` adds 12 and puts `openssl-sys` in the lockfile for Linux.
- OpenAI's published OpenAPI document (version 2.3.0) marks `max_tokens` as deprecated in favor of `max_completion_tokens` for chat completions, uses bearer authentication, and streams `chat.completion.chunk` objects terminated by `data: [DONE]`.

## Decision

**Transport.** Enable `ureq`'s `rustls` feature (keeping `default-features = false`): `ring` crypto provider and bundled Mozilla roots from `webpki-roots`. Certificate verification is always on and has no off switch. Both the local and remote destinations build their agent with `max_redirects(0)` (a 3xx response is returned and reported as a failure — `provider.rejected` for remote — never followed), `proxy(None)` and `http_status_as_error(false)`; a non-success response body is never read. Remote calls are bounded by a 15-second connect timeout and the existing 5-minute run deadline as a global timeout. Stream lines are capped at 1 MiB.

**One adapter.** `providers::local_openai` becomes `providers::openai_compatible`, serving a local destination (loopback `…/v1/chat/completions`, `max_tokens`, no auth, 0.8's `chat.failed` messages) and a remote one (`{https endpoint}/chat/completions`, `max_completion_tokens`, optional bearer token, normalized `provider.*` codes). The 0.8 canonical port is unchanged; the desktop shell only sees an opaque `CompletionTarget`. A provider trait is deferred to 0.12, the first protocol that actually differs.

**Profiles, binding and consent.** A remote profile (`ProviderProfileStore`, schema version 7) stores label, normalized HTTPS endpoint, model key, an optional `credential_id` foreign key to `credentials(id)` (`ON DELETE SET NULL`) and a consent record naming the exact endpoint it was granted for. Changing the endpoint clears the credential binding and the consent in the same statement; label/model changes keep them. Consent is granted only when the endpoint named in the request equals the stored one, and is reported only while it still matches. Profile management never performs network I/O.

**Dispatch.** A chat request carries a per-request `target`. For a remote target Rust checks, in order and before any network call or persistence: prompt bound, model equals the profile's model, consent valid for the current endpoint, then resolves the bound credential through `CredentialStore::resolve_secret`. The secret becomes a `BearerToken` (visible ASCII only, redacted `Debug`, no `Serialize`) placed only in a sensitive `Authorization` header value and dropped with the request. Nothing is retried automatically.

**Cancellation.** The reader thread observes the run's cancel flag: no request is sent if cancellation came first, and a streaming connection is dropped at the next received line, so a remote server stops generating. A reader blocked before the first byte is released only by that byte or the deadline; this is documented as best-effort.

Full behavioral detail lives in `openspec/changes/remote-openai-compatible-chat/design.md`.

## Consequences

- Lattice gains its first network egress to user-chosen hosts, confined to `lattice-core::providers::openai_compatible` and gated by explicit per-endpoint consent.
- Eight new crates, all permissively licensed (`rustls` Apache-2.0/ISC/MIT, `ring` Apache-2.0 AND ISC, `rustls-webpki`/`untrusted` ISC, `rustls-pki-types`/`zeroize` MIT/Apache-2.0, `subtle` BSD-3-Clause, `webpki-roots` CDLA-Permissive-2.0). `ring` compiles C/assembly with the toolchain SQLite already requires. No resident thread or process is added.
- OS-installed or enterprise root certificates and TLS-intercepting proxies are not supported in 0.11; environment proxies are ignored on purpose. Revisit through `platform-verifier` only for a concrete user need.
- Consent and credential binding are IPC actions, so a compromised WebView could perform the explicit update → bind → grant sequence itself. The 0.3 CSP is the current mitigation; a native confirmation is recorded for 0.26's egress abuse cases.
- The resolved secret is dropped after the request but not zeroized.
- Compatibility is claimed only for the pinned subset tested against fixtures and the documented OpenAI reference; other OpenAI-compatible services are listed as untested in `docs/remote-providers.md`.
