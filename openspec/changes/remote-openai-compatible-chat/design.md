# Remote OpenAI-Compatible Chat Design

## Ownership

`lattice-core` owns profile identity/validation, the destination-binding invariants, request authorization, secret-to-header preparation and all wire translation. `lattice-desktop` only wires commands, locks stores in a fixed order and spawns the existing orchestrator thread. Angular owns the Settings remote-provider section, the Chat selector/disclosure dialog and presentation of provenance; it never sees an endpoint-bound secret or makes a network request.

Module changes (no new crate):

- `crates/lattice-core/src/providers/profiles.rs` (new) — `ProviderProfile`, `ProviderConsent`, request DTOs, command constants, `REMOTE_PROVIDER_KEY`, endpoint/label/model validation, and `prepare_remote_target` (the remote authorization use case).
- `crates/lattice-core/src/providers/openai_compatible.rs` (renamed from `local_openai.rs`) — the only module that knows the OpenAI-compatible wire shape. Gains `OpenAiCompatibleTarget` (destination kind, request URL, optional bearer token), redirect/proxy/line-length/cancellation hardening and destination-specific error normalization.
- `crates/lattice-core/src/providers/completion.rs` — `ChatTarget`, `StartChatStreamRequest.target`; `run_chat_stream` takes an `OpenAiCompatibleTarget` instead of a bare endpoint and hands the reader thread the cancellation flag. `ChatRequest`, `ChatMessage`, `ChatStreamEvent`, `ChatFinishReason` are unchanged.
- `crates/lattice-core/src/storage/provider_profiles.rs` (new) — `ProviderProfileStore`, a fourth sibling store type sharing `lattice.sqlite3`, per `storage/mod.rs`'s guidance.
- `crates/lattice-core/src/storage/conversations.rs` — `start_assistant_message` takes the provider key instead of hard-coding `LOCAL_PROVIDER_KEY`.

## Contracts

```rust
pub const LIST_PROVIDER_PROFILES_COMMAND: &str = "list_provider_profiles";
pub const CREATE_PROVIDER_PROFILE_COMMAND: &str = "create_provider_profile";
pub const UPDATE_PROVIDER_PROFILE_COMMAND: &str = "update_provider_profile";
pub const DELETE_PROVIDER_PROFILE_COMMAND: &str = "delete_provider_profile";
pub const BIND_PROVIDER_CREDENTIAL_COMMAND: &str = "bind_provider_credential";
pub const GRANT_PROVIDER_CONSENT_COMMAND: &str = "grant_provider_consent";
pub const REVOKE_PROVIDER_CONSENT_COMMAND: &str = "revoke_provider_consent";

pub const REMOTE_PROVIDER_KEY: &str = "remote-openai-compatible"; // alongside 0.9's LOCAL_PROVIDER_KEY

pub struct ProviderProfile {
    pub id: String,                       // app-generated UUID v4
    pub revision: u64,
    pub label: String,
    pub endpoint: String,                 // normalized https base URL, e.g. https://api.openai.com/v1
    pub model_key: String,                // remote model id sent as `model`
    pub credential_id: Option<String>,    // reference only; never a secret
    pub consent: Option<ProviderConsent>, // present only when valid for `endpoint`
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}
pub struct ProviderConsent { pub endpoint: String, pub granted_at_unix_seconds: u64 }

pub struct CreateProviderProfileRequest { label, endpoint, model_key, credential_id: Option<String> }
pub struct UpdateProviderProfileRequest { id, expected_revision, label, endpoint, model_key }
pub struct DeleteProviderProfileRequest { id }
pub struct BindProviderCredentialRequest { id, expected_revision, credential_id: Option<String> }
pub struct GrantProviderConsentRequest { id, expected_revision, endpoint }
pub struct RevokeProviderConsentRequest { id, expected_revision }

#[serde(tag = "kind", rename_all = "camelCase", rename_all_fields = "camelCase")]
pub enum ChatTarget { Local, Remote { profile_id: String } }

pub struct StartChatStreamRequest { conversation_id: Option<String>, chat: ChatRequest, target: ChatTarget } // amended
```

Every mutating profile command returns the resulting `ProviderProfile` (delete returns `()`), mirroring `CredentialStore`. Update deliberately carries no credential: credential binding is its own explicit, revision-checked action so that "the endpoint changed" can never silently keep sending the same key.

New `AppError` constructors (all recoverable, fixed `&'static str` messages as every existing constructor):

| Code                        | Used for                                                                             |
| --------------------------- | ------------------------------------------------------------------------------------ |
| `provider.invalid`          | profile validation, unknown credential in a binding, unusable bearer bytes, model mismatch |
| `provider.not_found`        | unknown profile id                                                                   |
| `provider.conflict`         | stale `expected_revision`, consent endpoint differs from current endpoint            |
| `provider.consent_required` | remote request without consent valid for the current endpoint                       |
| `provider.auth_failed`      | HTTP 401/403                                                                         |
| `provider.rate_limited`     | HTTP 429                                                                             |
| `provider.timeout`          | connect/response/body deadline, HTTP 408                                             |
| `provider.unavailable`      | DNS/connect/TLS failure, HTTP 5xx                                                    |
| `provider.rejected`         | redirects (3xx), other 4xx, in-stream `error` payloads                               |

Local destination failures keep 0.8's `chat.failed` messages. Malformed/oversized/disconnected streams use `chat.failed` for both destinations — the shared outcome, not a vendor code.

## Schema (version 7)

```sql
CREATE TABLE provider_profiles (
  id TEXT PRIMARY KEY,
  revision INTEGER NOT NULL CHECK (revision >= 1),
  label TEXT NOT NULL,
  endpoint TEXT NOT NULL,
  model_key TEXT NOT NULL,
  credential_id TEXT REFERENCES credentials(id) ON DELETE SET NULL,
  consent_endpoint TEXT,
  consent_granted_at_unix_seconds INTEGER,
  created_at_unix_seconds INTEGER NOT NULL,
  updated_at_unix_seconds INTEGER NOT NULL
);
CREATE INDEX idx_provider_profiles_label ON provider_profiles(label COLLATE NOCASE, id);
```

- No secret column exists. `credential_id` is a real foreign key (every connection already enables `PRAGMA foreign_keys`): binding an unknown credential fails at insert/update and is reported as `provider.invalid`; deleting a credential through `CredentialStore`'s own connection unbinds it everywhere via `ON DELETE SET NULL`, without cross-store code.
- `consent_endpoint` records the exact destination consented to. `ProviderProfile.consent` is derived as `Some` only when `consent_endpoint == endpoint`, so a stale consent can never be reported or used even if a future code path forgot to clear it (the update path clears it anyway).
- Forward-only migration through the existing cascade with a pre-migration backup; a v6 fixture test proves existing credentials/conversations survive. At most `MAX_PROVIDER_PROFILES = 50` rows; lists are bounded by that cap.

## Profile invariants

- **Validation** (`validate_endpoint`): trimmed, at most 2048 bytes, printable ASCII only; scheme `https` (case-insensitive, normalized to lowercase); non-empty authority without `@`; host non-empty, optional numeric port 1–65535 (bracketed IPv6 allowed); no `?`, `#`, `\` or whitespace; trailing `/` removed. The normalized value must also parse as `ureq::http::Uri` so the transport can never reject a stored endpoint. Labels 1–120 characters, model keys 1–200 characters without control characters (both trimmed).
- **Destination binding**: `update` with a normalized endpoint different from the stored one sets `credential_id = NULL`, `consent_endpoint = NULL`, `consent_granted_at_unix_seconds = NULL` in the same statement that changes the endpoint. Label/model-only updates keep both.
- **Consent**: `grant` requires `expected_revision` to match *and* the request's `endpoint` (what the disclosure dialog displayed) to equal the stored endpoint; otherwise `provider.conflict`. `revoke` clears it. Every mutation bumps `revision` and `updated_at`.
- **No network**: the store has no HTTP dependency; a test binds a real `TcpListener`, points a profile at `https://127.0.0.1:<port>/v1`, runs every profile operation, and asserts `accept()` would still block.

## Dispatch and authorization

`start_chat_stream` keeps 0.8/0.9's shape: reserve the single active-run slot → check preconditions → persist → spawn. Preconditions now branch on `target`:

- `Local`: exactly 0.8's `authorize_chat_request` (prompt bound + owned model lease) and runtime endpoint → `OpenAiCompatibleTarget::local(endpoint)`, provider key `local-openai-compatible`, `ActiveChatRun.holds_local_model_lease = true`.
- `Remote { profile_id }`: lock `provider_profiles` → `get(profile_id)` → release; `prepare_remote_target(&profile, &chat, |id| credentials.resolve_secret(id))`:
  1. prompt bound (`MAX_PROMPT_CHARS`, shared with local);
  2. `chat.model_key == profile.model_key` else `provider.invalid`;
  3. `profile.consent` present (derived validity) else `provider.consent_required`;
  4. if `credential_id` is set, lock `credentials` → `resolve_secret` → release; map to `BearerToken` (ASCII-trimmed, 1–4096 bytes, visible ASCII only, else `provider.invalid`). Resolver errors (`credential.not_found`/`unavailable`/`unsupported`) propagate unchanged;
  5. `OpenAiCompatibleTarget::remote(format!("{endpoint}/chat/completions"), token)`; provider key `remote-openai-compatible`; `holds_local_model_lease = false`.

Stores are locked one at a time, never nested (settings → released; profiles → released; credentials → released; conversations), so no lock-order deadlock is introduced. Only after every precondition passes are messages persisted, exactly as 0.9 requires. A profile without a bound credential is dispatched unauthenticated (for HTTPS endpoints that need no key); a `401` then surfaces as `provider.auth_failed`.

`unload_model` refuses only when the active run `holds_local_model_lease`.

## Adapter: one protocol, two destinations

| Aspect              | Local (0.8, unchanged unless noted)              | Remote (new)                                         |
| ------------------- | ------------------------------------------------ | ---------------------------------------------------- |
| URL                 | `{runtime endpoint}/v1/chat/completions`         | `{profile endpoint}/chat/completions`                |
| Transport           | loopback `http`                                  | `https` via rustls + bundled Mozilla roots           |
| Authorization       | none                                             | `Authorization: Bearer …` when bound, marked sensitive |
| Output bound field  | `max_tokens` (LM Studio docs)                    | `max_completion_tokens` (OpenAI 2.3.0 spec)          |
| Timeouts            | `timeout_recv_body = STREAM_DEADLINE`            | `timeout_connect = 15 s`, `timeout_global = STREAM_DEADLINE` |
| Redirects / proxy   | now 0 / none (was 10 / env)                      | 0 / none                                             |
| HTTP errors         | `chat.failed` ("local model runtime …")          | normalized `provider.*` table above                  |

Shared translation rules (the pinned subset):

- Body: `{"model", "messages": [{"role","content"}], "stream": true, <limit field>: MAX_OUTPUT_TOKENS}` serialized with `serde_json`. Assistant messages with empty text are omitted from the wire body only (a reply that failed before its first token would otherwise be sent as an empty assistant turn, which some compatible servers reject); canonical storage is untouched. No `stream_options`, `n`, sampling or tool fields.
- `http_status_as_error(false)`: the status is inspected directly; a non-2xx response body is never read, so no vendor error text can leak.
- SSE: lines read through a bounded reader (`MAX_STREAM_LINE_BYTES = 1 MiB`; a longer line or invalid UTF-8 fails the run with `chat.failed` without buffering further). `data:` with or without one following space is accepted; comment (`:`), `event:`, `id:`, `retry:` and blank lines are ignored. `data: [DONE]` completes as `stop`. A payload with an `error` member fails the run (`provider.rejected` remote, `chat.failed` local). Only `choices[0]` is read; `delta.content` and `delta.refusal` text become canonical deltas. `finish_reason` `length` → `MaxOutputTokens`; every other value → `Stop` (0.11 sends no tools, so `tool_calls` cannot legitimately occur; `content_filter` is recorded in the conformance matrix as reported as `stop`).
- A body read timing out (`io::ErrorKind::TimedOut`) maps to `provider.timeout` remote and to 0.8's documented "Generation took too long and was stopped." locally; any other read error to "disconnected before finishing".

### Cancellation (best-effort remote)

The orchestrator is unchanged: it checks the shared flag every `ORCHESTRATOR_POLL_INTERVAL` and emits `Cancelled` without waiting. The reader thread now also receives the flag: it returns without sending if cancelled before dispatch, and after every received line it returns without forwarding anything if cancelled, dropping the response and closing the socket — which stops a remote server from continuing to generate billable tokens. A reader blocked before the first byte can still only be released by the first chunk or the deadline; this residual cost is documented as "best-effort", matching the roadmap's "honest best-effort remote cancellation".

## Tauri shell

`DesktopState` gains `provider_profiles: Mutex<ProviderProfileStore>` (opened in `.setup()` against the same database path after `credentials`, so its FK target table exists) and `ActiveChatRun.holds_local_model_lease`. Seven commands delegate to the store; `delete_provider_profile` is allowed while a run is active because the run already holds its own resolved target. A new `allow-provider-profiles` permission is added to `capabilities/default.json` and both inventory tests.

## UI

- **Settings → Providers & credentials → Remote providers**: add form (label, endpoint, model, credential native select); a card per profile with endpoint, model, credential select (binds on change), consent badge ("Approved for this endpoint" / "Not approved") with a Revoke action, an inline Edit form that warns "Changing the endpoint removes the credential binding and approval", and Remove with the existing confirmation dialog pattern.
- **Chat**: a provider native select in the header (`Local model` + one option per profile). The readiness empty state only applies to the local target. Sending through a profile without consent opens a disclosure dialog naming the endpoint, the model, the credential label and that the full visible conversation (including earlier local replies) will be sent; "Allow and send" grants consent for that exact endpoint and then sends, "Cancel" sends nothing. Assistant messages show their provenance (`Local · model` / `Remote · model`).
- Selection is presentation state in `ChatStore` (default local), not persisted.
- Volt UI `select[voltNativeSelect]`, `volt-card`, `volt-badge`, `volt-button`, `volt-form-field`/`volt-input` and the existing Quartz `DialogService` confirmation pattern are reused; no new primitive.
- Browser fallback simulates profiles, binding, consent and a remote streamed reply with the same preconditions and error codes.

## Dependency admission

**TLS for `ureq`.** Enable `ureq`'s `rustls` feature (`ring` provider + `webpki-roots`), keeping `default-features = false` (no `gzip`, `json`, `cookies`). Measured by toggling features and diffing `Cargo.lock` on this workspace, 2026-09-16:

| Option                                 | New crates | Notes |
| -------------------------------------- | ---------- | ----- |
| `rustls` (accepted)                    | 8: `ring`, `rustls`, `rustls-pki-types`, `rustls-webpki`, `subtle`, `untrusted`, `webpki-roots`, `zeroize` | Same code path on macOS/Linux/Windows; no system library; roots pinned by the lockfile. Does not trust OS-installed/enterprise roots. |
| `rustls` + `platform-verifier`         | 18 (adds `rustls-platform-verifier`, `jni`, `rustls-native-certs`, `webpki-root-certs`, `openssl-probe`, …) | Honors OS trust store; more than doubles the addition for a need no v1 journey states. |
| `native-tls-no-default`                | 12 (adds `native-tls`, `openssl`, `openssl-sys`, `der`, …) | Uses Security.framework on macOS but requires system OpenSSL headers on Linux, burdening 0.14's preview builds. |
| Hand-rolled TLS / shelling out to curl | —          | Rejected: security-critical protocol code, or secrets in argv. |

Licenses (from each crate's manifest): `rustls` Apache-2.0 OR ISC OR MIT; `ring` Apache-2.0 AND ISC; `rustls-webpki`/`untrusted` ISC; `rustls-pki-types`/`zeroize` MIT OR Apache-2.0; `subtle` BSD-3-Clause; `webpki-roots` CDLA-Permissive-2.0 (data). All maintained under the rustls project/briansmith with frequent releases. Runtime cost: TLS state exists only during a remote request; no background thread, no resident process. `ring` builds C/assembly with the existing `cc` toolchain `rusqlite`'s bundled SQLite already requires. Removal path: switch the feature back off; only `openai_compatible.rs` touches TLS configuration. The OS-trust-store limitation is recorded in the egress policy; revisit through `platform-verifier` only with a concrete user need.

No other dependency: endpoint validation is hand-written string validation plus `ureq::http::Uri` (already compiled), secrets need no zeroization crate for a request-scoped buffer (documented residual: bytes are dropped, not wiped).

## Security notes and residual risks

- Secret lifetime: resolved inside `start_chat_stream`, moved into the reader thread as `BearerToken` (redacted `Debug`, no `Serialize`), converted once into a sensitive `HeaderValue`, dropped when the request ends. No global cache.
- `ureq` debug logging redacts every non-allowlisted header; Lattice's own code never formats a token, request URL with secrets (none exist — bearer only) or response body.
- Endpoint changes cannot carry a key to a new host implicitly. A compromised WebView could still issue the explicit update → bind → grant sequence itself, because consent and binding are IPC actions; a native (non-WebView) confirmation is out of scope for 0.11 and is listed for 0.26's egress abuse cases. The 0.3 CSP (no remote script, no `unsafe-eval`) is the current mitigation.
- Remote endpoints are user-chosen; Lattice does not block private-network hosts. HTTPS certificate validation against Mozilla roots is always on; there is no "disable verification" option.

## Documentation sources

Checked 2026-09-16:

- OpenAI OpenAPI document `https://app.stainless.com/api/spec/documented/openai/openapi.documented.yml` (`info.version 2.3.0`): server URL, `/chat/completions`, bearer security scheme, `CreateChatCompletionStreamResponse`/`ChatCompletionStreamResponseDelta` schemas, `finish_reason` enum, `max_tokens` deprecation text, `data: [DONE]` examples.
- `https://developers.openai.com/api/docs/guides/error-codes`: 401/403/429/500/503 meanings.
- `ureq` 3.4.1 source in the local Cargo registry: `config.rs` (redirect/proxy/status defaults and timeouts), `tls/rustls.rs` (provider and root selection), `util.rs` (`DebugHeaders` redaction).

Other OpenAI-compatible services are not claimed compatible; the conformance matrix lists them as untested.

## Verification

- Rust: profile validation/normalization table, store CRUD/revision/FK-unbind/endpoint-change/consent tests, v6→v7 migration fixture, no-network-on-profile-operations listener test, `prepare_remote_target` refusals (model, consent, credential states, bad token bytes) with `FakeSecretStore`, and loopback fixture tests of the remote destination: bearer header and `max_completion_tokens` observed by the fixture, 401/403/429/500/503/3xx/404 normalization with a secret-bearing body proving no body text leaks, in-stream `error`, oversized line, cancel-before-dispatch (fixture receives no connection), cancel mid-stream closes the connection. Two protocol fixtures (local and remote destinations) share one canonical history to prove the same `ChatRequest` produces equivalent canonical events.
- Opt-in, network-gated (`#[ignore]` + `LATTICE_REMOTE_SMOKE=1`): unauthenticated HTTPS request to `https://api.openai.com/v1` → `provider.auth_failed`; an authenticated streaming smoke additionally gated on `LATTICE_REMOTE_SMOKE_ENDPOINT`/`_MODEL`/`_API_KEY`, recorded as pending when no maintainer credential is available.
- TypeScript/Angular: wire decoders, provider-profile fallback, remote chat fallback (consent/model/credential refusals, provenance), Playwright journeys for Settings profile management and Chat local → remote (consent dialog) → local with reopened history.
- Full `bun run check`, web build, E2E, OpenSpec strict validation and native `--no-bundle` build.
