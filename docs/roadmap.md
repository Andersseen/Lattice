# Lattice roadmap to 1.0

Engineering plan, 2026-09-06. Baseline: unpublished `0.1.0` foundation at `2e8eb7e`. Status updated 2026-09-12: `0.2` is archived, the `0.3` desktop security baseline is implemented on the active branch, `0.4` persistent settings is implemented with native GUI restart evidence pending before archive, `0.5` ModelRuntime discovery is implemented with real llmster smoke evidence pending before archive, `0.6` llmster lifecycle is implemented on the active branch with a real successful start/stop cycle and the P1 profile pending before archive, and `0.7` installed model management is implemented on the active branch with qualified-candidate confirmation and the P2 profile pending before archive (this machine's llmster installation cannot wake without a one-time GUI first run; see `docs/verification/0.6-llmster-lifecycle.md` and `docs/verification/0.7-installed-model-management.md`). Future release entries remain planned until their OpenSpec changes are accepted and verified. No dates, tags or published releases are created by this plan.

The destination is a lightweight, local-first, provider-agnostic desktop agent workspace. A small Rust agent core coordinates replaceable inference, explicit tools, portable skills, MCP, memory, Spaces and basic tasks. It does not reproduce LM Studio, Hermes, an IDE or a distributed agent platform.

## Reading and execution contract

- [Repository assessment](v1/repository-assessment.md): source evidence, actual behavior and debts.
- [Capability map](v1/capability-map.md): current/target status and release ownership.
- [Architecture sequence](v1/architecture-sequence.md): responsibility map, dependency graph, contracts and security.
- [Definition of v1](v1/definition-of-v1.md): product acceptance, platform support, testing/CI, performance gates and common DoD.
- [SDD workflow](development/spec-driven-development.md): how one release scope becomes one accepted OpenSpec change.

Each minor is one branch and one reviewable PR, including a thin usable UI where necessary. Cross-layer changes are acceptable when they close that single objective; adding a second capability is not. No speculative crates, screens or registries. The common DoD and architecture constraints apply to every checklist below. **Documentation** names the future spec capability to add/update, not a spec that already exists.

Before each minor: explore actual prerequisite state and upstream versions → proposal → behavioral spec delta → design → tasks → implement → verify → archive. Do not open all future changes now. `openspec/specs/foundation/spec.md` remains current behavior truth; this plan defines intended sequence. If a scope cannot fit one coherent review, split and amend the roadmap before implementation. No silent PR expansion.

## Release index

| Release | Status                 | One outcome                                                 |
| ------- | ---------------------- | ----------------------------------------------------------- |
| 0.2     | Done, archived         | Verifiable application IPC contracts and error handling.    |
| 0.3     | Done, archive/evidence | Restrictive desktop security baseline.                      |
| 0.4     | Done, archive/evidence | Persistent non-secret settings.                             |
| 0.5     | Done, archive/evidence | llmster discovery through a consumed ModelRuntime boundary. |
| 0.6     | Done, archive/evidence | Approved, owned runtime lifecycle.                          |
| 0.7     | Done, archive/evidence | Installed local model list/load/unload.                     |
| 0.8     | Planned                | First local streaming chat and cancellation.                |
| 0.9     | Planned                | Provider-independent conversation persistence.              |
| 0.10    | Planned                | Native secure credential provisioning.                      |
| 0.11    | Planned                | Remote OpenAI-compatible chat.                              |
| 0.12    | Planned                | Anthropic adapter.                                          |
| 0.13    | Planned                | Gemini adapter.                                             |
| 0.14    | Planned                | Scoped workspace reads and permissions.                     |
| 0.15    | Planned                | Bounded agent loop with registered read tools.              |
| 0.16    | Planned                | Approved conflict-checked file edits.                       |
| 0.17    | Planned                | Approved foreground command execution.                      |
| 0.18    | Planned                | Portable skills.                                            |
| 0.19    | Planned                | Trusted local MCP tool servers.                             |
| 0.20    | Planned                | Explicit workspace memory and retrieval.                    |
| 0.21    | Planned                | Persistent Spaces combining existing capabilities.          |
| 0.22    | Planned                | Manual tasks and execution results.                         |
| 0.23    | Planned                | Safe one-shot local scheduling.                             |
| 0.24    | Planned, optional      | Optional task Kanban projection.                            |
| 0.25    | Planned                | Measured resource and cleanup budgets.                      |
| 0.26    | Planned                | Execution security qualification.                           |
| 0.27    | Planned                | Verified macOS distribution artifacts.                      |
| 0.28    | Planned                | First-use and recovery qualification.                       |
| 1.0     | Planned                | Promotion of a qualified release candidate.                 |

0.24 may be skipped without renumbering. All other listed capabilities are required. Provider adapters are separate minors; filesystem and terminal are distinct authorization surfaces. Persistence precedes history, credentials precede remote requests, and permissions precede the agent. Provider abstraction starts with the first real completion consumer, avoiding a later local-chat rewrite.

## 0.2 — Application Runtime Foundation

**Status:** done and archived as `openspec/changes/archive/2026-09-07-application-runtime-foundation`; permanent behavior lives in `openspec/specs/application-api/spec.md`.

**Objective:** make the existing application boundary deterministic and resistant to contract drift.

**Why now:** every later command needs one wire/error convention. **Dependencies:** current foundation.

**Scope:** Rust DTO generation into `packages/types`, checked command/request/result inventory, one bridge decoder/error normalizer, owned safe error messages, latest-refresh result ordering, truthful browser fallback types, strict Angular template and test-source type checks. Only application composition consumed by current metadata.

**Non-goals:** settings, SQLite, process/event/provider frameworks, ModelRuntime, UI redesign, new native capabilities.

**Boundaries / contracts:** core DTO/error modules, Tauri metadata command, shared types, Angular API/store, checks. Native metadata and separate presentation web fallback; `AppError` with stable code, bounded safe message, recoverability and optional correlation; typed application result and checked command mapping. Use the generation decision in the architecture sequence, with dependency admission in the ADR.

**Security / resources:** redact before serialization/logging; no raw internal source in UI, no resident work.

**Testing:** Rust serialization fixtures consumed by TS; malformed payload/unknown error; two out-of-order refreshes; generation drift; Tauri dispatch/serialization integration plus recorded native metadata smoke. Enable strict template/test-source checks without weakening types.

**Acceptance:**

- [ ] Unregenerated Rust DTO or inconsistent command mapping fails verification.
- [ ] Unknown errors become one safe result; stores contain no second normalizer; stale refresh cannot overwrite newer data.
- [ ] Native/browser states remain distinguishable; strict checks pass and IPC evidence goes beyond a direct Rust function call.

**Documentation:** update `foundation`; add `application-api` spec, DTO/error ADR, pinned OpenSpec validation and contributor check instructions during implementation.

**Exit:** the next command can use one checked application boundary and every 0.2 criterion passes.

## 0.3 — Desktop security baseline

**Status:** implementation done on the active branch. The branch has restrictive CSP, explicit main-window command permission, fatal startup exit policy, advisory workflow/policy and automated verification. Native WebView denial evidence and P0 idle measurement remain recorded follow-up evidence before final archive, not a new product minor.

**Objective:** restrict the shipped WebView and command exposure before adding native capabilities.

**Why now:** process/network features must not inherit disabled CSP. **Dependencies:** 0.2.

**Scope:** production CSP for bundled Angular assets/required IPC, separate dev allowances, explicit application-command/window permissions, minimal core/log grants, safe unsuccessful startup exit and P0 resource capture. Introduce scheduled dependency/advisory checks and policy.

**Non-goals:** generalized authorization engine, credentials, file tools, runtime, new product routes.

**Boundaries / contracts:** Tauri security/startup/config, narrowly necessary Angular nonce/style compatibility, CI. Trusted main window reads metadata; unauthorized callers cannot; fatal boot failure is visible and exits unsuccessfully.

**Security / resources:** no remote executable UI, wildcard remote IPC or production dev-server allowance. Any style exception must be narrow and verified; no `unsafe-eval`. P0 establishes measured idle baseline.

**Testing:** production native rendering/IPC, blocked script/connect, unauthorized command/window, startup failure harness; browser smoke separately.

**Acceptance:**

- [ ] CSP is non-null and native Home/System/IPC still work.
- [ ] Disallowed origins/windows cannot invoke commands; blocked resource loads are observed in the WebView.
- [ ] Fatal boot failure, P0 evidence and dependency-check policy are verified.

**Documentation:** `desktop-security` spec, baseline ADR, security policy and P0 record.

**Exit:** native baseline passes before process or provider access is exposed.

## 0.4 — Persistent application settings

**Status:** implementation done on the active branch. Rust temporary database restart coverage, migration rollback, newer-schema refusal, Tauri build and browser Settings smoke pass. Live native GUI restart evidence remains before final archive.

**Objective:** retain validated non-secret preferences across restart.

**Why now:** runtime selection needs safe persistence. **Dependencies:** 0.3.

**Scope:** SQLite under Rust in OS app-data, versioned migration transactions, settings read/update/reset and minimal UI. First real consumers: appearance and idle-unload preferences. Pre-migration backup and unsupported-schema policy; later minors add only their own keys.

**Non-goals:** conversations, universal repository framework, cloud sync, secrets, workspace files.

**Boundaries / contracts:** core settings, Rust SQLite adapter, Tauri app paths/IPC, Settings UI. `AppSettings`, revision, schema version, structured storage failure; choose a maintained lightweight SQLite binding with documented alternatives.

**Security / resources:** user-local data, no SQL IPC/secrets; bounded queries and serialized writes, no indexing worker.

**Testing:** temporary DB, round-trip/restart, invalid values, revision conflict, migration rollback, newer-schema refusal and unwritable storage; UI save/retry.

**Acceptance:**

- [ ] Settings survive restart; invalid/conflicting writes preserve the old revision.
- [ ] Failed migration preserves data/backup; a newer schema is never overwritten.
- [ ] Frontend cannot supply arbitrary SQL or storage paths.

**Documentation:** `application-settings`, `local-storage`, SQLite/migration ADR and recovery guide.

**Exit:** settings and schema evolution are reliable enough for runtime configuration.

## 0.5 — ModelRuntime discovery

**Status:** implementation done on the active branch. Fixture CLI/status probes, persisted approved metadata, endpoint health, Tauri build and browser Models smoke pass. Opt-in real llmster smoke evidence remains before final archive.

**Objective:** report whether a supported llmster installation and endpoint are available.

**Why now:** lifecycle needs verified identity and honest unavailable states. **Dependencies:** 0.4.

**Scope:** management discovery/status port with actual setup/status UI consumer; known/user-selected absolute executable, bounded version/status probes, loopback health and stored approved metadata. Executable probes require explicit approval before invocation.

**Non-goals:** daemon/server startup, models, installers/downloads, generic runtime plugins, unused inference traits.

**Boundaries / contracts:** core ModelRuntime discovery, isolated llmster adapter, Tauri selection, Models/setup UI. `RuntimeInstallation`, statuses missing/unsupported/stopped/running/unreachable/unknown; executable identity and separate daemon/server observations.

**Security / resources:** no executable lookup from a project directory; approved read-only probe only; validate loopback resolution/reject redirects. Bounded timeout/output, no polling service.

**Testing:** pinned CLI/HTTP fixtures for absent/incompatible/malformed/timeout states, paths with spaces, daemon present/server absent; installed-runtime manual discovery without a model.

**Acceptance:**

- [ ] Missing runtime shows guidance without starting/downloading anything; unsupported versions are explicit.
- [ ] Executable identity and distinct daemon/server status survive refresh/restart.
- [ ] UI consumes management DTOs; tested CLI/API versions are recorded.

**Documentation:** `model-runtime` discovery spec, ADR 0005 clarification and compatibility record.

**Exit:** verified discovery/status supports lifecycle without llmster coupling in Angular.

## 0.6 — llmster lifecycle

**Status:** implementation done on the active branch. Disposable-fixture start/stop/ownership/cancellation tests, OpenSpec/contract/Rust/TypeScript/web-build/E2E/native-build checks, and real-CLI failure-path evidence (clean no-orphan behavior on a genuine llmster wake failure) pass. A real successful owned start/stop cycle and the ten-cycle P1 profile remain before archive; see `docs/verification/0.6-llmster-lifecycle.md`.

**Objective:** start/stop approved runtime resources without disrupting external workloads.

**Why now:** models need a reachable owned-or-attached runtime. **Dependencies:** 0.5; security baseline 0.3.

**Scope:** daemon/server start/stop, bounded health wait, owned/attached identity, idempotency, cancel and shutdown cleanup. Reconcile daemonization; launcher PID is not assumed to be daemon identity.

**Non-goals:** model loading, system/login services, arbitrary terminal execution, process-supervisor platform.

**Boundaries / contracts:** llmster lifecycle adapter, core runtime transitions, Tauri shutdown, Models controls. `RuntimeOwnership`, operation ID/outcome and observed state; changed executable identity revokes approval.

**Security / resources:** fixed argv, sanitized environment, loopback server; no kill-by-name or attached-resource stop. Ownership at launch does not prove exclusive use forever: if external activity is observed or safe shutdown cannot be established, refuse automatic stop and report the condition. No eager runtime. P1 measures ten cycles.

**Testing:** disposable CLI/process fixtures for duplicate start, occupied endpoint, timeout, crash, daemonized identity, attachment and shutdown; real llmster smoke without model.

**Acceptance:**

- [ ] Repeatable lifecycle never automatically kills attached/ambiguous resources.
- [ ] Failure/cancel produces one terminal result and accurate state/ownership.
- [ ] Ten cycles/exit leave no owned orphan; P1 and real-runtime evidence exist.

**Documentation:** `model-runtime` lifecycle deltas, process ownership ADR, troubleshooting and P1.

**Exit:** lifecycle/cleanup pass against a qualified runtime version.

## 0.7 — Installed model management

**Status:** implementation done on the active branch. Disposable-fixture list/load/unload/ownership/cancellation tests, OpenSpec/contract/Rust/TypeScript/web-build/E2E/native-build checks pass; real-CLI `--help` confirmation for `ls`/`ps`/`load`/`unload` and a genuine no-orphan environmental-failure path (the same headless daemon-wake gap 0.6 recorded) pass. Real `ls`/`ps` JSON field-name capture, qualified small Qwen/Gemma candidate confirmation and the P2 load/unload profile remain before archive; see `docs/verification/0.7-installed-model-management.md`.

**Objective:** select and manage one installed local model.

**Why now:** streaming needs known model capacity/state. **Dependencies:** 0.6.

**Scope:** installed versus loaded inventory, list/select/load/unload, one managed model slot, loading/busy/failure state. Select small Qwen/Gemma candidate profiles and manual acquisition guidance; chat qualification follows in 0.8.

**Non-goals:** model downloader/marketplace, inference/quantization internals, whole-family compatibility claims.

**Boundaries / contracts:** ModelRuntime adapter/management, Rust load state/lease, settings, Models UI. `ModelDescriptor`, `LoadedModel`, load options, explicit context limit and safe failure. Use observed model IDs, not guessed paths.

**Security / resources:** no external model auto-unload; replacement requires releasing an idle owned model. Account for runtime JIT; P2 load/unload/memory pressure measurement with exact profiles.

**Testing:** installed-but-unloaded, duplicate/busy load, insufficient memory, unload-in-use, external load, runtime crash; real candidate load/unload.

**Acceptance:**

- [x] Inventory accurately separates installed/loaded; unload releases owned model state.
- [x] Conflicting load cannot evict active/external work.
- [ ] Profile IDs/configuration, runtime version and measured load/unload evidence are recorded; missing-model guidance is actionable. (Pending: real catalog read and P2 profile; see verification doc.)

**Documentation:** `local-models` spec, model qualification matrix, P2 load/unload record.

**Exit:** a selected installed model is demonstrably manageable and ready for streaming qualification.

## 0.8 — Local streaming chat

**Objective:** stream and cancel text chat using the selected local model.

**Why now:** the first inference consumer justifies a minimal provider port. **Dependencies:** 0.7.

**Scope:** canonical text request/events, Rust OpenAI-compatible local transport, in-memory Chat UI, channel lifecycle and cancellation; model lease, context/output/deadline limits and bounded event buffers. Plain text rendering suffices.

**Non-goals:** persistence, remote traffic, tools/AgentRuntime, embeddings, vendor-managed chat state.

**Boundaries / contracts:** core completion/provider port, Rust adapter, Tauri channel, Chat. Request ID/sequence/role/text/model capabilities, terminal outcome and cancel request; no llmster fields in the provider port.

**Security / resources:** approved loopback only; inert output; one stream, no auto-retry/fallback. Clean up leases/listeners. Complete P2 active-stream measurement.

**Testing:** fragmented/malformed stream, disconnect, late/duplicate terminal, cancel before first token/midstream; fixture UI and real native local chat on both candidate profiles.

**Acceptance:**

- [ ] Incremental text and cancel work with exactly one terminal outcome per request.
- [ ] Stale events cannot reach a new run; crash/error frees the active slot.
- [ ] Named chat profiles pass smoke; context/output/queue limits and P2 evidence are verified.

**Documentation:** `chat-streaming`, `providers` text spec, provider/streaming ADR and P2 completion.

**Exit:** usable local chat works through a neutral provider contract with bounded cancellation.

## 0.9 — Conversation persistence

**Objective:** reopen and continue provider-independent conversations.

**Why now:** remote substitution/agents need canonical records. **Dependencies:** 0.8 and 0.4.

**Scope:** create/list/reopen/delete, canonical text messages and generation outcomes, stable IDs, pagination, restart reconciliation. Persist partial output as interrupted using bounded checkpoints rather than every token.

**Non-goals:** memory extraction, Spaces, sync, tools, automatic retry after restart.

**Boundaries / contracts:** core conversations, SQLite migration/repository, history IPC/UI. `Conversation`, `Message`, generation status and provider/model provenance; vendor IDs are never primary keys.

**Security / resources:** private local data/deletion controls; no credentials/headers in history; bounded pages and checkpoint writes.

**Testing:** migration/reopen/delete/order, crash during stream, disk failure, large history pagination and native restart.

**Acceptance:**

- [ ] Restart retains ordered messages; interrupted output is labeled, never resent automatically.
- [ ] Transactional delete preserves other conversations.
- [ ] Large history remains paginated in storage/UI and has no provider-specific ownership.

**Documentation:** `conversations` spec, retention and migration guidance.

**Exit:** local conversation identity/history survives restart and is ready for provider switching.

## 0.10 — OS-secure credentials

**Objective:** provision/revoke credentials without exposing values to Angular.

**Why now:** must precede remote traffic. **Dependencies:** 0.3 and 0.4.

**Scope:** Rust-owned native secure input on supported macOS, OS keychain adapter, references, availability and delete/replace controls. Angular initiates native entry and receives status only; no network test yet.

**Non-goals:** remote requests, OAuth, custom cryptography, plaintext fallback, Angular password forms.

**Boundaries / contracts:** Rust credential port/OS adapter, Tauri native dialog, Settings reference UI. `CredentialRef`, label/provider binding, available/locked/missing/unsupported; no get-secret IPC.

**Security / resources:** secret only in native entry/store/Rust request preparation; no value in DOM, IPC responses, SQLite, logs or argv. No long-lived global secret cache. Unsupported platforms fail closed.

**Testing:** fake store contract; native secure-entry/keychain smoke, locked/revoke/restart and seeded-secret checks across IPC/logs/DB; verify absence of Angular input/state paths.

**Acceptance:**

- [ ] Native entry/replace/delete yield accurate reference status after restart.
- [ ] No secret leaks; locked/unavailable storage never falls back to plaintext.
- [ ] Tested native secure-input dependency/implementation and platform limits are documented.

**Documentation:** `credentials` spec, secure entry/storage ADR, setup/security guide.

**Exit:** Rust can resolve an authorized reference while the frontend cannot obtain the value.

## 0.11 — Remote OpenAI-compatible chat

**Objective:** switch the same conversation to a remote compatible provider.

**Why now:** validate the first actual provider substitution. **Dependencies:** 0.8, 0.9, 0.10.

**Scope:** provider/model profiles with credential references, explicit HTTPS endpoint, context-transfer consent, compatible remote streaming and normalized auth/rate/timeout errors. Selection affects the next request; pin the supported compatible protocol subset.

**Non-goals:** Ollama/llama.cpp/vLLM lifecycle adapters, provider marketplace, auto-failover, agent tools, additional API surfaces.

**Boundaries / contracts:** Rust profile/selection use case with local/remote consumers, compatible adapter, profile storage, Settings/Chat selector. `ProviderProfile`, tested capabilities and destination-bound consent.

**Security / resources:** HTTPS remote/loopback HTTP local, no unsafe redirects; endpoint change invalidates consent/credential binding. No request on selection alone; bounded requests and honest best-effort remote cancellation.

**Testing:** shared canonical history via two protocol fixtures, auth/rate/redirect/unsupported cases, no-network-on-selection, redaction; bounded opt-in real remote smoke.

**Acceptance:**

- [ ] Local → remote → local retains history and sends only on explicit request/consent.
- [ ] Adapter details stay outside domain/UI; failures/cancel follow shared outcomes.
- [ ] Tested endpoint/protocol subset is documented without universal compatibility claims.

**Documentation:** `providers` remote deltas, egress policy and conformance matrix.

**Exit:** local/remote chat use the same application/history contracts without redesign.

## 0.12 — Anthropic adapter

**Objective:** use Anthropic with canonical conversations and existing UI behavior.

**Why now:** a distinct protocol tests domain independence. **Dependencies:** 0.11.

**Scope:** Rust text/stream/error translation, model capability metadata, credential binding and conformance entry. Explore official current API docs and pin the tested API version.

**Non-goals:** vendor-specific UI/memory/skills, tool execution, cache optimization, extra providers.

**Boundaries / contracts:** one adapter and narrow registration/settings option; retain canonical requests/events/history, extend only for demonstrated shared semantics.

**Security / resources:** inherit secret/egress/stream limits, no background service or telemetry.

**Testing:** message conversion, fragmented stream, usage/stop reasons, auth/rate/cancel, shared text suite and opt-in real smoke.

**Acceptance:**

- [ ] Existing conversation streams via Anthropic and returns to local without lost content/identity.
- [ ] Unsupported content is explicit; raw vendor errors/secrets do not leak.
- [ ] Conformance and native smoke pass for a named model/version.

**Documentation:** `providers` Anthropic scenarios and supported-provider/setup matrix.

**Exit:** Anthropic passes text conformance; tool conformance is added in 0.15.

## 0.13 — Gemini adapter

**Objective:** use Gemini with canonical conversations and existing UI behavior.

**Why now:** completes the deliberately small provider set. **Dependencies:** 0.11; independent of 0.12 implementation.

**Scope:** Rust text/stream/error translation, supported capability metadata, secure profile option and conformance. Pin API version after official-source exploration.

**Non-goals:** multimodal/voice, grounding, Vertex dependency, extra provider families.

**Boundaries / contracts:** one adapter/registration option; canonical messages/outcomes unchanged. Necessary continuation metadata is namespaced and never canonical identity.

**Security / resources:** inherited egress/stream limits; credentials never enter logged URLs/query strings.

**Testing:** system/message conversion, stream fragmentation, blocked/no-content/error responses and cancel; shared text suite and real smoke.

**Acceptance:**

- [ ] Local/Gemini switching preserves history with explicit capability errors.
- [ ] Blocked responses terminate predictably instead of hanging.
- [ ] Conformance/native smoke pass for a named model with no secret exposure.

**Documentation:** `providers` Gemini scenarios, supported matrix and recovery guidance.

**Exit:** three provider families have independent text-contract evidence.

## 0.14 — Workspace scope and read permissions

**Objective:** select a workspace and read files under Rust-enforced grants.

**Why now:** establish a real permission consumer before tools. **Dependencies:** 0.9, 0.3 and 0.4.

**Scope:** persistent workspace identity/root, native directory selection, explicit read grants, bounded list/read operations, workspace rules as untrusted context and minimal preview UI. Explicitly associate conversations with a workspace for later scoped retrieval; unassigned history stays unassigned. Revalidate at access time, including symlink/path replacement. Add Linux/Windows core compile/path tests.

**Non-goals:** agent loop, writes/terminal, watchers/indexing, Git UI, multi-root/enterprise workspaces.

**Boundaries / contracts:** Rust workspace/permission logic and filesystem adapter, Tauri picker, Context UI, storage/IPC. `WorkspaceRef`, rules, `PermissionGrant`, scope/revision, allow/deny and auditable read outcome.

**Security / resources:** deny by default; exclude `.git`, known secret files and app data, expose exclusions; root change revokes grants. Bound text reads; no recursive repository attachment.

**Testing:** traversal/symlink/reparse/root substitution, revocation, binary/large/denied files, disjoint workspaces and native picker/deny; rules cannot authorize access.

**Acceptance:**

- [ ] Access remains within approved scope under path changes; Rust denies before I/O.
- [ ] Preview shows included rules/files and limits; no automatic secret/tree attachment.
- [ ] Revocation affects the next read; platform tests/unavailable states pass.

**Documentation:** `workspaces`, `permissions`, filesystem threat-model ADR and CI platform matrix.

**Exit:** agent read tools can reuse proven operations and one policy engine.

## 0.15 — Bounded agent loop and read tools

**Objective:** complete a prompt → approved read-tool call → result → answer cycle.

**Why now:** providers/history and permissions now have real consumers. **Dependencies:** 0.14, 0.11 and 0.9; certify 0.12/0.13 adapters before their agent mode is enabled.

**Scope:** small Rust ToolRegistry wrapping only existing list/read operations; argument/result validation; canonical tool-call/result message blocks, deterministic sequential agent loop, visible approvals/step outcomes and run cancellation. Extend the three adapters with the same bounded tool protocol subset; no provider-specific agent loop. Persist step identity/outcomes using conversation storage. One run, at most eight tool calls and fixed deadline.

**Non-goals:** writes, terminal, MCP, planner framework, parallel tools/agents, automatic repair loops or automatic failed-tool retries.

**Boundaries / contracts:** core agent/tool modules, provider tool translation, conversation schema, Tauri run/events and existing Chat tool-step UI. `AgentRun`, `ToolDefinition`, `ToolCall`, `ToolResult`, `PermissionRequest`; running/awaiting-approval/completed/cancelled/failed/interrupted transitions. No tool executes before complete valid arguments and approval.

**Security / resources:** model content cannot grant permissions; per-call policy check, one-use approvals bound to exact arguments; bounded output/context/steps/time, P3 profile. Advertise agent support only for certified model/adapter combinations.

**Testing:** deterministic scripted provider and existing scoped-read fixture: answer without tools, valid call, invalid/unknown call, deny, cancellation at each phase, repeated tool request, context/step exhaustion and restart. Shared adapter tool suite and qualified real local/remote smoke.

**Acceptance:**

- [ ] A real qualified local model and each enabled remote adapter complete one approved read cycle through the same loop.
- [ ] Invalid/denied/cancelled calls perform no read; limits terminate predictably; completed state cannot change.
- [ ] A crashed run is interrupted, never resumed into an unrecorded side effect; P3 shows bounded resources.

**Documentation:** `agent-runs`, `tools`, provider/conversation deltas, small-agent ADR, tested tool-capability matrix and P3.

**Exit:** one reliable bounded loop consumes ToolRegistry and permissions without provider-owned agent state.

## 0.16 — Approved file edits

**Objective:** create/update workspace text files only after reviewing the exact change.

**Why now:** extends an already tested read-only agent with one side effect. **Dependencies:** 0.15 and 0.14.

**Scope:** create/update tools, before/after diff, per-call write approval, expected file revision/content hash and atomic replacement; result records previous/new revisions and conflict. Recheck scope/grant/parent identity immediately before commit. User can inspect changes without executing them.

**Non-goals:** recursive delete/move, unrestricted patch shell, auto-commit, general undo/version-control platform, unattended writes.

**Boundaries / contracts:** scoped filesystem adapter, existing tools/permissions, diff/approval UI, audit/history. `ProposedFileChange`, expected revision, one-use approval, applied/conflict/denied outcome.

**Security / resources:** approval covers exact bytes/path; modifications invalidate approval. Protect workspace/root/secret exclusions; race-safe file operations, bounded file/diff sizes.

**Testing:** changed file after preview, symlink/parent replacement, create collision, denial, revocation, partial write failure and crash boundary; UI diff plus real temporary workspace edit.

**Acceptance:**

- [ ] Only the approved bytes/path are written; stale previews return conflict with no overwrite.
- [ ] Denied/revoked/out-of-scope writes leave files intact under path-race fixtures.
- [ ] Audit and conversation identify the applied revision or uncertain failure; no automatic replay.

**Documentation:** `tools` write scenarios, `permissions` deltas and edit/recovery guide.

**Exit:** agent edits are reviewable, scope-enforced and conflict-safe before more powerful process tools.

## 0.17 — Foreground command execution

**Objective:** run an exact user-approved command and inspect its bounded result.

**Why now:** developer workflows need execution, with a distinct stronger trust decision. **Dependencies:** 0.16, 0.14 and 0.6 cleanup patterns.

**Scope:** one foreground process tool with executable/argv/cwd/env preview, per-call approval, captured stdout/stderr, exit code, timeout and process-tree cancellation. Show that the process uses the user's OS privileges. Default execution bypasses a shell; explicitly selected shell invocation requires approval of its full command content.

**Non-goals:** terminal emulator/PTY, sandbox/container promise, persistent shell, elevated privileges, background jobs, unattended execution.

**Boundaries / contracts:** Rust execution adapter, existing tools/policy/audit, narrow result UI. `CommandProposal`, one-use approval, process identity and completed/failed/timed-out/cancelled/unknown outcome.

**Security / resources:** cwd is not a security sandbox; arbitrary approved commands can access outside it. Never label terminal as read-only. No inherited credential environment; bound output and deadline; terminate descendants using platform adapter and report failure honestly.

**Testing:** argv quoting/path spaces, rejected proposal, modified arguments, environment isolation, large output, child/grandchild processes, timeout and cancel/exit races on platform fixtures.

**Acceptance:**

- [ ] No command runs without exact foreground approval; changed executable/arguments require new approval.
- [ ] Captured output is bounded; process tree is cleaned up or unresolved state blocks another command and is visible.
- [ ] UI explains OS authority; scheduled execution cannot access this tool.

**Documentation:** `tools` process scenarios, execution threat-model ADR, terminal limits and cross-platform evidence.

**Exit:** a foreground command has accountable authorization, outcomes and cleanup without implying sandboxing.

## 0.18 — Portable skills

**Objective:** import/select human-readable skills and include them in agent context.

**Why now:** a working agent/context path can consume instructions. **Dependencies:** 0.15, 0.14 and 0.4.

**Scope:** portable folder containing `SKILL.md` with name/description metadata, local discover/import by native selection, enable/disable/select and read-only preview; copy approved skill content into app-managed storage with source/version/content hash. Document authoring in an ordinary editor and reimport/update behavior. Selected enabled skills are snapshotted into each run.

**Non-goals:** marketplace/Git downloader, automatic learning, dependency installation, executing skill scripts or arbitrary linked files.

**Boundaries / contracts:** Rust skill parser/import/context assembler, filesystem selection adapter, skill metadata storage and Skills UI. `SkillManifest`, source/hash/revision, selection and bounded instruction content. A skill folder remains portable across providers.

**Security / resources:** preview before import/use; skills never grant permissions. Reject symlink escapes, oversized content and unsupported metadata; local files are inert text. No background scanning; load only selected skills within context budget.

**Testing:** valid/malformed metadata, duplicate/reimport IDs, missing file, escaped links, disabled skill, changed revision and context-budget overflow; same skill under local/remote provider fixtures.

**Acceptance:**

- [ ] Imported skill can be enabled/selected/disabled, persists and influences the next run's recorded context.
- [ ] Provider change preserves skill identity/instructions; disabled skills are excluded.
- [ ] Import cannot run code or expand access; unsupported resources are reported, not silently loaded.

**Documentation:** `skills` spec, portable format/authoring guide and context-order rules.

**Exit:** portable selected skills are a provider-independent input to the existing agent.

## 0.19 — Local MCP tools

**Objective:** configure a trusted stdio MCP server and call its tools through existing policy.

**Why now:** ToolRegistry and process cleanup exist. **Dependencies:** 0.15, 0.6, 0.14; 0.10 for optional server credential references.

**Scope:** server executable/argv/env-reference configuration, explicit process trust, on-demand connect/initialize/version negotiation, bounded tool discovery/pagination, namespaced ToolRegistry entries, invocation, status, cancellation and disconnect. Support stdio tools only with a pinned tested MCP protocol subset.

**Non-goals:** remote HTTP/OAuth, server marketplace/download, prompts/resources/sampling platform, automatic `npx` installs, unattended MCP.

**Boundaries / contracts:** Rust MCP client/transport adapter, existing process/policy/tools, server storage, MCP status/approval UI. `McpServerConfig`, connection status, server-namespaced tool/schema revision and tool outcome.

**Security / resources:** approve executable trust separately from each tool action; schema change invalidates grants; metadata is untrusted. Server is not OS-sandboxed. Sanitized env, bounded stderr/protocol/tool counts, lazy startup, no reconnect storm, no orphan after disconnect/exit.

**Testing:** disposable stdio fixture for handshake mismatch, malformed JSON, discovery pages, duplicate names, schema change, process exit, hanging tool, denial and cancellation; one real trusted server smoke without external account prerequisite.

**Acceptance:**

- [ ] Configure/connect/discover/approve/call/disconnect works through the same ToolRegistry and audit path.
- [ ] Unapproved/changed tools never dispatch; server failure is visible and isolated from other tools.
- [ ] Disconnect/cancel respects limits and ownership; no tool/server is auto-installed or enabled for scheduling.

**Documentation:** `mcp-tools` spec, protocol/support matrix, MCP threat model and configuration guide.

**Exit:** a trusted local MCP server supplies bounded, permission-checked foreground tools.

## 0.20 — Explicit workspace memory

**Objective:** retain and retrieve user-selected workspace facts without vector infrastructure.

**Why now:** conversations and workspace scope are persistent. **Dependencies:** 0.9, 0.14 and 0.4; 0.15 to consume memories in agent runs.

**Scope:** create/edit/delete explicit memories with workspace/provenance/revision, user pinning and bounded lexical search across workspace memories and conversations. Deterministic context selection: pinned entries first, then case-insensitive term matches by match count, updated time and ID; preview included items and truncation. No embedding dependency.

**Non-goals:** automatic learning/extraction, vector/graph store, global cross-workspace retrieval, silent memory mutation by the agent.

**Boundaries / contracts:** Rust memory/retrieval/context assembler, SQLite queries/migration, Memory/context UI and IPC. `MemoryEntry`, provenance, query/result/excerpt and selected revision.

**Security / resources:** workspace filter applied before ranking, explicit delete, untrusted memory text, remote disclosure through existing policy. Bound candidate count, query length and context bytes; no full-history load.

**Testing:** predictable ranking/ties, no-match, update/delete, workspace isolation, budget truncation and malicious instructions; restart/retrieval UI and same results across provider fixtures.

**Acceptance:**

- [ ] Explicit memories survive restart and can be removed from future retrieval.
- [ ] Same query/snapshot gives deterministic bounded context and never leaks another workspace's records.
- [ ] UI shows included memory/provenance; changing provider preserves memory without granting new access.

**Documentation:** `memory` spec, retrieval/context policy, retention/deletion guidance.

**Exit:** explicit useful memory is persistent, inspectable and provider-independent.

## 0.21 — Persistent Spaces

**Objective:** group existing capabilities into one reusable workspace configuration.

**Why now:** the referenced capabilities now exist; a Space will not be an empty abstraction. **Dependencies:** 0.12, 0.13, 0.17, 0.18, 0.19 and 0.20, including their prerequisites.

**Scope:** create/edit/delete/select Space with name, workspace, provider/model profile, selected skills, MCP references, memory/context preferences and permission references. One active Space; resolve/snapshot configuration at run start. Existing standalone conversations stay usable and can be explicitly assigned to a Space. Tasks attach in 0.22.

**Non-goals:** teams/tenancy, nested Spaces, sync, automatic provider fallback, duplicated skills or memories.

**Boundaries / contracts:** Rust Space composition/validation, reference storage, Spaces switcher/editor, existing application APIs. `Space`, configuration revision, resolved run context and missing-reference state.

**Security / resources:** references do not grant new authority. A Space switch cannot mutate an active run, eagerly start MCP/models or transmit data; removed configuration becomes unavailable visibly. Deleting a Space unassigns history, not cascading deletion of shared data.

**Testing:** restart, missing/deleted profile/skill/server, cross-Space isolation, active-run switching, revoked permission and provider change preserving context; two-Spaces E2E using fixtures plus native smoke.

**Acceptance:**

- [ ] Each Space restores its selected configuration and shows unresolved references.
- [ ] Switching provider preserves workspace/skills/memory/history; next execution records the resolved revision.
- [ ] Switching/deleting Space neither broadens permissions nor starts work/deletes shared records.

**Documentation:** `spaces` spec, reference/deletion/context ADR and user workflow guide.

**Exit:** one coherent persistent Space composes the v1 capabilities without duplicating their ownership.

## 0.22 — Manual tasks and results

**Objective:** save an instruction as a task, execute it and inspect every attempt.

**Why now:** tasks can consume an existing Space and bounded run. **Dependencies:** 0.21 and 0.15.

**Scope:** task create/edit/archive, explicit Ready action, run-now, cancel, result review/done and manual retry creating a new attempt. One global execution slot is shared with chat; busy tasks remain Ready, never run invisibly. Snapshot resolved Space/config and permission revision per attempt; durable result links to conversation/run.

**Non-goals:** scheduler, Kanban, task dependencies, bulk automation, implicit retry, task-specific parallel runtime.

**Boundaries / contracts:** Rust task state machine/use case, SQLite task/attempt migration, Tasks detail/list, reuse AgentRuntime/IPC. `Task`, `TaskAttempt`, immutable result; Backlog → Ready → Running → Review → Done, Running → Failed, Failed → Ready on explicit retry. Attempt outcomes distinguish cancelled/failed/interrupted; these map to task Failed with a reason.

**Security / resources:** normal foreground tool prompts apply; edits during execution do not alter its snapshot; audit links task/run/grants. Paginated attempts/results, no duplicate agent service.

**Testing:** transition table, edit-during-run, double dispatch, busy slot, cancellation, failed/retry attempt identity, restart in Running and durable result consistency; task E2E.

**Acceptance:**

- [ ] Run-now creates one durable attempt; success enters Review and only user acceptance marks Done.
- [ ] Crash marks an attempt interrupted and task Failed without automatic rerun; retry creates a separate ID.
- [ ] Results/configuration/permission outcomes remain inspectable across restart and task archival.

**Documentation:** `tasks` spec, task/attempt state diagram, retention/result/retry guide.

**Exit:** persistent manual tasks have deterministic state and trustworthy attempt history.

## 0.23 — Safe one-shot scheduling

**Objective:** execute a preapproved safe task once at a future time while Lattice is open.

**Why now:** scheduling must reuse durable task execution and permissions. **Dependencies:** 0.22, 0.14 and 0.6; remote tasks additionally require 0.10/0.11.

**Scope:** one future schedule per task, timezone-aware input resolved to UTC, stored original zone/offset, enable/disable, transactional dispatch claim and schedule status. Separate task-specific grant/configuration snapshot, rechecked at dispatch. Allowed unattended operations: generation and built-in scoped reads only. One application instance owns the scheduler/DB dispatch lease.

**Non-goals:** recurrence/cron, OS wake/login daemon, runs while app is closed, unattended writes/terminal/MCP, automatic retries/catch-up or distributed scheduling.

**Boundaries / contracts:** Rust scheduler clock/dispatch use case, task DB and native app lifecycle, schedule editor/status. `Schedule`, due UTC/zone, scheduled/blocked/dispatched/missed/cancelled state; unique task-schedule occurrence claim and dedicated grant. Editing task/Space after scheduling invalidates authorization until review.

**Security / resources:** deny/revocation/locked keychain/missing runtime yields blocked, never a hanging approval modal. Explicit remote destination/read-scope consent. One timer, no polling loop; busy slot waits at most five minutes, then missed. App startup/resume marks past-due unclaimed work missed for explicit Run now/reschedule; no automatic catch-up. Already claimed interrupted runs are not replayed.

**Testing:** fake clock, DST gap/overlap requiring user disambiguation, timezone change, sleep/resume/restart, duplicate process/claim, revocation/edit, busy grace timeout, credential lock and crash around dispatch. Explicitly test that terminal/MCP/write are unavailable.

**Acceptance:**

- [ ] Approved safe task dispatches once while open/available; result/attempt is persisted.
- [ ] Closed/sleeping/overdue tasks show missed status and require explicit action; no exactly-once side-effect claim after crash.
- [ ] Revocation or changed inputs block dispatch; retry is a new explicit attempt and forbidden tools never run.

**Documentation:** `task-scheduling`, scheduler/clock/claim ADR, unattended threat model and visible app-open limitation.

**Exit:** one-shot scheduling has predictable missed/blocked/failure behavior and cannot bypass foreground permissions.

## 0.24 — Optional Kanban projection

**Objective:** view the same tasks in six simple status columns.

**Why now:** task transitions are stable and can be projected without a second workflow engine. **Dependencies:** 0.22 and 0.23 for schedule badges.

**Scope:** Backlog, Ready, Running, Review, Done, Failed columns; task cards link to existing detail/results. Accessible move controls call the same allowed task transitions; dragging is not required. Scheduled/blocked/missed/cancelled/interrupted are badges/reasons, not additional columns.

**Non-goals:** custom columns, sprints, dependencies, assignments, bulk dispatch, persistent duplicate board state.

**Boundaries / contracts:** Angular task view and existing task APIs; Rust changes only if a missing projection query is required. No new task lifecycle contract or database board table.

**Security / resources:** moving a card never grants permission or starts execution. Running is controlled only by dispatch; Review/Done reflect existing outcomes. Paginate/filter cards using existing queries.

**Testing:** projection mapping and allowed/rejected transitions, keyboard move/detail flow, scheduler badge visibility; no new runtime fake.

**Acceptance:**

- [ ] Board/list show the same tasks/states after refresh/restart.
- [ ] Illegal moves fail through the existing state machine; moving to Ready does not execute work.
- [ ] If this requires a new workflow engine or threatens hardening, record it as deferred and skip this release.

**Documentation:** `task-views` spec only if implemented; record implement/defer decision in roadmap/capability map.

**Exit:** the optional view passes, or an explicit deferral permits 0.25 with the full task list workflow intact.

## 0.25 — Resource and lifecycle qualification

**Objective:** meet measured non-model overhead and cleanup budgets in the integrated workflow.

**Why now:** only the composed product reveals model/MCP/agent/task interactions. **Dependencies:** 0.23 and all required feature branches; 0.24 only if shipped.

**Scope:** reproducible P4 profile/soak, instrument process/queue/lease counts and bounded diagnostics, fix measured eager startup/leaks/unbounded queues; enforce idle unload and cleanup policy already introduced. Record model profiles, CPU, native/WebView/runtime/MCP/model memory separately.

**Non-goals:** new features, telemetry service, benchmark marketing, inference optimization, broad speculative refactors.

**Boundaries / contracts:** existing lifecycle/resource owners and minimal read-only diagnostics UI/API, test harness/docs. `ResourceSnapshot` with timestamp/process ownership and unavailable metrics explicitly labeled; no guessed exact GPU attribution.

**Security / resources:** redact paths/content/secrets from diagnostics; manual local export only. Apply the 1 GiB target/2 GiB ceiling, CPU/cleanup/regression gates and uncertainty rules in Definition of v1.

**Testing:** 60-minute mixed-use soak, repeated load/unload and runs, three-MCP fixture bound, cancel/exit/sleep/resume/blocked schedule, large-history fixture; compare P0–P3 on the same reference environment.

**Acceptance:**

- [ ] P4 report names exact configuration, method and observed results; all defined ceilings/cleanup gates pass.
- [ ] No eager processes or accumulating queues/listeners/runs remain after cleanup; external resources are untouched.
- [ ] Each optimization is backed by a measured defect and relevant regression check; failures block exit.

**Documentation:** `resource-lifecycle` spec, P4 evidence, qualified-model/resource matrix and revised budgets only through an explicit recorded decision.

**Exit:** measured integrated overhead/cleanup meet the documented release gates.

## 0.26 — Execution security qualification

**Objective:** verify the complete trust boundary with adversarial end-to-end cases.

**Why now:** isolated safeguards must compose correctly before distribution. **Dependencies:** 0.25 and every shipped capability.

**Scope:** threat-model reconciliation and targeted fixes for failed abuse cases: WebView/IPC, path races, changed approvals, prompt injection, MCP executable/schema changes, credential leakage, remote egress, scheduled grants and audit completeness. Dependency/advisory/license inventory and explicit dispositions. This release certifies existing controls, not introduces them late.

**Non-goals:** general OS sandbox, certification claims, new auth platform, new tools/transports or unrelated UX cleanup.

**Boundaries / contracts:** only affected existing trust-boundary modules/tests; preserve contracts unless a documented security fix requires a versioned change. Publish security assumptions, known limitations and a concrete private reporting contact/path.

**Security / resources:** a seeded secret must not appear in UI/IPC/logs/DB/export; arbitrary terminal/MCP remain explicitly trusted foreground executables. No instrumentation that retains sensitive prompts or causes unbounded audit growth.

**Testing:** abuse-case matrix with direct Rust boundary tests and real native denial evidence; malicious skill/MCP output cannot authorize I/O; changed task configuration/revoked grant cannot run. Repeat resource checks only where fixes affect them.

**Acceptance:**

- [ ] Every applicable threat-model case has passing evidence; no unresolved critical/high exploitable finding in shipped flows.
- [ ] Dependency findings have versioned dispositions; secrets/egress/denials/audit pass, not just happy paths.
- [ ] Limitations and vulnerability reporting route are concrete and tested for usability.

**Documentation:** `execution-security` spec consolidating existing invariants, threat-model verification, SECURITY/reporting and dependency/license inventory.

**Exit:** the shipped trust boundary has no unaddressed release-blocking security finding.

## 0.27 — Desktop distribution

**Objective:** produce an installable, signed and verifiable macOS release artifact.

**Why now:** signed packaging must include the qualified core. **Dependencies:** 0.26; packaging exploration can start earlier without claiming qualification.

**Scope:** actual `.app`/DMG bundle, proper icons/metadata, signature/notarization and verification, checksums/license notices, manual upgrade/backup/restore and uninstall behavior. Declare tested macOS minimum/current versions. CI release/scheduled native builds on Linux/Windows; compile/core tests remain required per support policy.

**Non-goals:** release publication in this planning session, automatic updater, app stores, bundled llmster/model redistribution, claiming stable Linux/Windows installers.

**Boundaries / contracts:** Tauri packaging, narrowly needed data migration/backup hooks, release CI, support docs. Artifact/version identity, supported platform matrix and schema compatibility; no new agent behavior.

**Security / resources:** signing credentials in protected CI only, never forks/repo; verify signature/notarization/checksum. Package no dev server, Node/Bun backend, model weights or private fixture data.

**Testing:** clean-machine install/open/uninstall, path/app-data permission, upgrade from previous shipped schema, failed migration and compatible-backup restore, downgrade refusal. Three-platform compile/native-build evidence; macOS secure entry and real IPC from the packaged app.

**Acceptance:**

- [ ] A signed/notarized DMG installs/opens on both declared macOS versions and passes signature checks.
- [ ] Upgrade preserves data; incompatible downgrade is refused safely; uninstall data-retention behavior is documented.
- [ ] Linux/Windows preview checks pass with honest unsupported capability states; artifact contains no forbidden runtime/secrets.

**Documentation:** `desktop-distribution`, release/upgrade/backup guide, support matrix, signing runbook and OSS notices.

**Exit:** real installable artifacts and data migration/recovery are qualified; `--no-bundle` alone cannot pass.

## 0.28 — First-use and recovery qualification

**Objective:** let a new user complete the entire supported v1 journey with actionable recovery.

**Why now:** final UX validation must use packaged behavior. **Dependencies:** 0.27 and all mandatory release checklists.

**Scope:** one coherent setup flow linking existing runtime/model/provider/Space screens; clear missing-runtime/model, permission/credential, task-missed and failure guidance. Fix blockers discovered by the Definition of v1 journey, keyboard/accessibility checks and long-history UI. Consolidate user/support docs and P5 evidence; no visual-system migration.

**Non-goals:** new capability, model installer, recurrence, broad UI redesign, mandatory Volt UI/Angular Movement adoption.

**Boundaries / contracts:** Angular onboarding/navigation/status presentation, existing APIs, docs/native verification. No parallel domain logic; every setup status reflects existing Rust state. Runtime installation remains an explicit external official step.

**Security / resources:** first remote send discloses context transfer; credentials use native entry; task UI states app-open requirement. No remote assets/telemetry added for onboarding. P5 profiles signed release behavior.

**Testing:** clean-user journey through local chat, provider switch, skill/MCP, Space, approved tool, memory and scheduled result; offline/locked/missing/failed paths, keyboard navigation and backup/restore using packaged build. Automated fixtures plus reproducible native manual evidence.

**Acceptance:**

- [ ] A new user completes every mandatory journey step using docs without undocumented developer setup.
- [ ] Empty/loading/denied/failed/missed/recovery states are accessible and truthful; no raw secret or stack trace leaks.
- [ ] P5, compatibility matrices, meaningful CI and all required release evidence pass; no core-flow architectural TODO remains.

**Documentation:** `first-use` spec, final user/troubleshooting/support guides, Definition of v1 evidence and release notes draft.

**Exit:** scope is frozen and the candidate is ready for `1.0.0-rc.N` qualification.

## 1.0 — Stable workflow promotion

**Objective:** publish only the behavior already qualified in the release candidate.

**Why now:** all required capabilities and quality gates are complete. **Dependencies:** 0.28 and passing RC evidence; 0.24 is optional.

**Scope:** RC blocker fixes only, evidence reconciliation, final version/artifact metadata and stable promotion. Pre-1.0 minors deliver capabilities; RCs freeze scope; stable 1.0 is a support/quality commitment. No tag or publication is performed by this documentation session.

**Non-goals:** new features, stealth dependency changes, broad refactors, completing deferred Kanban during promotion.

**Boundaries / contracts:** release metadata/docs/pipeline; implementation changes only as separately reviewed blocker fixes with affected checks rerun. Freeze documented public/domain behavior and migration/support contract.

**Security / resources:** protected signing/release credentials; verify final artifact provenance, no regressions from qualified security/resource evidence.

**Testing:** exact candidate commit CI, signed artifact install/upgrade, essential local/remote/tool/scheduled workflow and P5 comparison; repeat affected qualification for every RC fix.

**Acceptance:**

- [ ] Mandatory Definition of v1 checklist has linked evidence and no blocker or unverified required check.
- [ ] Signed artifacts/checksums/support/model matrix/upgrade docs match the released version.
- [ ] Current OpenSpec specs match shipped behavior; completed changes are archived and deferred work stays explicit.

**Documentation:** release notes/changelog, support/security policy, final capability statuses and OpenSpec archive reconciliation.

**Exit:** `1.0.0` is promotable with a complete, useful and bounded desktop workflow; later features require new scopes.

## Critical path and parallel opportunities

The mandatory local path is 0.2 → 0.3 → 0.4 → 0.5 → 0.6 → 0.7 → 0.8 → 0.9. Remote credentials/compatible transport (0.10–0.11) and workspace permissions (0.14) join at the agent (0.15). Anthropic/Gemini, writes/terminal, skills, MCP and memory join at Spaces (0.21), followed by tasks → scheduling → resources → security → packaging → first-use → RC → 1.0. See the [graph and exact dependency discussion](v1/architecture-sequence.md#dependency-graph).

After 0.11, Anthropic and Gemini can be separate parallel branches. After their prerequisites, skills/MCP/memory can proceed independently; filesystem edits precede terminal. Docs, packaging research and profile fixture preparation can proceed alongside feature work. Merges must preserve accepted contracts; no agent should implement several minors at once by default.

## Explicitly post-v1

Advanced learning/multi-agent/planning, Vertex/Wisp integration, extra runtimes/providers, advanced Kanban, remote MCP/OAuth, recurring/powerful unattended automation, remote execution, server mode, web client, cloud sync/collaboration. No commitment to an infinite roadmap. Agentix and UI-library adoption require actual consumers and dependency justification.

## Next implementation handoff

**Only 0.8 — Local streaming chat.** 0.2 is archived; 0.3, 0.4, 0.5, 0.6, and 0.7 are implemented on the active branch with their recorded evidence limits (see `docs/verification/`). Start from the accepted `local-models` change and ADR 0011. Read AGENTS, current architecture/specs, this scope and linked v1 constraints; explore actual prerequisite state (the managed model slot's owned/attached semantics, llmster's OpenAI-compatible completion endpoint); then create one OpenSpec change for `chat-streaming`/`providers` before coding. Do not implement persistence, remote providers, tools, or credentials as part of `0.8`.
