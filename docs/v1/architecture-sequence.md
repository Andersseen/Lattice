# Architecture and sequencing for v1

Status: planned target, not implemented architecture. Read the [assessment](repository-assessment.md), [release scopes](../roadmap.md) and current [foundation spec](../../openspec/specs/foundation/spec.md) together.

## Responsibility map

| Layer                      | Owns                                                                                                                           | Must not own                                                                               |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| Angular                    | Lazy feature routes, rendering, input validation for UX, signals/computed for interaction state, typed application calls       | Domain policy, SQL, credentials, filesystem/process/network adapters, scheduling decisions |
| `packages/types`           | Exported wire types and small presentation-only types, consumed by application API                                             | Native code, provider SDKs, duplicate hand-maintained Rust DTOs                            |
| Tauri / `lattice-desktop`  | Shell composition, native dialogs, app paths, secure OS facilities, command exposure, channels and application shutdown        | Provider business rules, a second agent loop or a second permissions engine                |
| Rust / `lattice-core`      | Application use cases, domain state transitions, context assembly, run limits, authorization, task scheduling policy           | Tauri/WebView assumptions, vendor-shaped conversations or prompts                          |
| Rust adapters              | llmster lifecycle/model API, provider protocols, MCP transport, OS credential implementation, SQLite repository implementation | Product ownership of skills/memory/prompts; their job is translation and I/O               |
| llmster                    | Local inference, model loading and runtime internals                                                                           | Lattice conversations, agent orchestration, MCP registry, skills or memory                 |
| SQLite                     | Local records, migrations, transactions and bounded querying under Rust APIs                                                   | Secret values or code execution; Angular never gets direct SQL access                      |
| Vertex / Wisp              | Optional future editor / execution integrations                                                                                | Required application services or dependencies on Lattice core                              |
| Agentix                    | Optional reusable utilities if a real consumer justifies them                                                                  | Required application host or replacement of the small Rust agent core                      |
| Volt UI / Angular Movement | Optional explicitly selected UI/motion libraries                                                                               | Required architecture milestones; neither is necessary to deliver v1                       |

Keep new domain modules inside `lattice-core` initially. Isolate vendor code in Rust adapter modules; create a dedicated crate only when an active dependency/platform boundary warrants it in the minor's design. Do not scaffold every box now. No persistent Node/Bun backend, Electron, Nx, global frontend store, inference engine, tokenizer or vector infrastructure.

## Two separate runtime boundaries

```text
Angular → typed application API → Tauri → Rust application use cases
                                            │
                               Agent runtime / context assembly
                              /      |       |       |        \
                          Skills   Memory   Tools  Spaces   Conversations
                                            │
                                    Permission policy
                                            │
                                  Provider completion port
                              /              |              \
                  OpenAI-compatible      Anthropic         Gemini
                        │
                  local endpoint
                        │
                     llmster ← llmster adapter ← ModelRuntime management
```

`ModelRuntime` manages discovery, daemon/server status and model inventory/load/unload. `Provider` handles inference requests, streaming and tool-call translation. Local completions use the OpenAI-compatible provider transport and a lease on a model managed by ModelRuntime. A remote provider requires no local runtime. They are complementary boundaries, not interchangeable implementations of one oversized trait.

0.5 introduces the management contract with a real read-only discovery/status consumer; it does not create unused inference traits. 0.8 introduces the provider port with the first local streaming consumer. 0.11 validates replacement using a remote endpoint. This avoids a special local-chat path that would need redesign later. Lattice does not delegate agent loops, stateful conversations, skills or MCP to llmster even if its API offers them.

Current implementation status as of 2026-09-12: the 0.2 application API boundary is archived; 0.3 desktop security, 0.4 settings/local storage, 0.5 ModelRuntime discovery, 0.6 llmster lifecycle (owned/attached start-stop), 0.7 installed model management, and 0.8 local streaming chat (the `Provider` boundary's first consumer) are implemented on the active roadmap branch with their recorded evidence limits. The next product boundary to add is 0.9 conversation persistence.

## Contracts that preserve portability

- **Wire DTOs (0.2):** Rust is authoritative for serialized application data; generate committed TS bindings in `packages/types` and fail CI on regeneration diff. Planned generator: `ts-rs`, subject to a focused dependency/serde compatibility check in the 0.2 ADR. TS-only `web` fallback state stays a presentation union around native metadata. Do not edit generated DTOs. Command names/request/result mapping must share a checked inventory; type generation alone does not catch command spelling or runtime JSON errors. Keep a small bridge decoder for untrusted IPC data, verified against Rust serialization fixtures; do not add an independent schema platform.
- **Errors (0.2):** stable code, owned bounded safe message, recoverability and optional correlation ID. Internal sources remain Rust-side and are redacted before logging. The application API normalizes once into a typed result; UI stores render it. Unknown/malformed wire payloads produce one transport error, not a cast or a second normalization chain.
- **Settings (0.4):** schema-versioned non-secret preferences; update validates before transactional persistence. Credential references arrive with their consumer in 0.10.
- **Streaming (0.8):** request/run identity, monotonically increasing sequence, text deltas and exactly one terminal outcome (`completed`, `cancelled`, `failed`). Register the channel before dispatch; late events cannot mutate a newer run. A reconnect obtains state rather than replaying a generation automatically. 0.15 adds tool events only when needed.
- **Provider (0.8/0.11–0.15):** canonical messages, ordered text/tool-call/tool-result blocks, selected provider/model, bounded context/output limits, explicit capabilities and normalized failure reasons. Capability flags describe tested model/adapter combinations, never a whole family by assumption. Missing tool support disables agent execution visibly. Provider-specific IDs are metadata, not canonical primary keys. Switching provider retains history/context and requires a new run; no silent remote fallback or background transcript transfer.
- **Runs (0.15):** one active run globally; sequential tool calls; cancellation/deadline/step/output limits are Rust decisions. Persist messages and step outcomes before the next side effect. Terminal state is immutable. Denied tools never execute. An interrupted external side effect is recorded as unknown, never silently retried.
- **Context:** deterministic order: application safety rules, workspace rules, selected skill instructions, explicit memory, conversation and current request. Treat file/skill/MCP/model text as untrusted content, never authority to expand permissions. Budget context in Rust; show truncation. Preserve original stored history; unsupported blocks cause an explicit compatibility error, not destructive rewriting.
- **Identity:** application-owned stable IDs for workspaces, conversations, runs, tool calls, skills, memories, Spaces and tasks. IDs are added with consumers, not a universal object system. A Space stores references and configuration, not duplicate copies of these domains. Every execution records the resolved configuration revision.

The [ts-rs project](https://github.com/Aleph-Alpha/ts-rs) provides Rust-to-TypeScript export. Generation is a build/test concern; the compatibility decision and version must be documented before adoption, not installed during this planning session.

## Security decisions before capabilities

1. **0.3, desktop:** restrictive production CSP, trusted local assets and minimal IPC destinations; dev-only HMR allowances stay separate. No remote executable UI or remote WebView IPC authority. Verify actual application command restrictions and window labels; plugin capabilities alone are insufficient. Tauri documents that registered application commands are available by default unless explicitly restricted. [Tauri capabilities](https://v2.tauri.app/security/capabilities/), [CSP](https://v2.tauri.app/security/csp/).
2. **0.6, processes:** user approves the verified executable and loopback endpoint. Separate daemon and HTTP-server states. Record whether each resource was started by Lattice or merely attached. Stop only provably owned resources, no global kill/unload. Uncertain ownership becomes attached/unknown and requires explicit reconciliation. No startup/login service, curl-pipe-shell installer or implicit downloads.
3. **0.10, secrets:** Rust-owned native secure entry and OS credential store; Angular sees only `CredentialRef` and availability. No raw secret in Angular state, DOM, IPC responses, SQLite, config or logs. A locked/unavailable keychain blocks remote use; no plaintext fallback. Native secure entry is required on supported macOS, not an Angular password form. Other platforms fail closed until a tested adapter exists.
4. **0.14–0.17, tools:** grants bind subject, operation, canonical resource, lifetime and policy revision. A selected directory is not permission to its entire filesystem. Deny symlink/reparse escapes and path substitution at access time; protect `.git`, credential files and Lattice storage. Use atomic conflict-checked writes. Terminal executable/argv/cwd/environment are shown exactly before approval. An arbitrary command is not sandboxed by `cwd`; v1 requires approval for every interactive process and forbids terminal in unattended tasks. No claim of a secure general-purpose shell sandbox.
5. **0.19, MCP:** stdio only for v1; trust/approve executable and sanitized environment separately from tool-call grants. No `npx` download on connect. Tool names are server-namespaced; changed schemas invalidate grants. Tool annotations do not establish safety. A local server is an executable with the user's OS authority, not a filesystem-confined built-in tool. No unattended MCP calls in v1.
6. **0.23, scheduling:** dedicated task grant and configuration snapshot; revocation wins at dispatch. Unattended v1 permits text generation and built-in scoped reads only. Writes, terminal and MCP require a foreground run. Remote tasks require an explicit destination/data-scope grant and an already unlocked credential. Never display a modal and silently continue in background.

Audit records store IDs, operation, resource summary, policy decision, timestamp and outcome; omit secrets and full prompts/outputs. Task results and conversation text are user data, retained separately with deletion controls. Auditing is local and inspectable, not tamper-proof against the OS user. SQLite is not encrypted by default; document reliance on OS account/disk protection.

## Runtime and resource rules

Start model/MCP processes on demand. One loaded model and one run at a time are the v1 product limit. Acquire a model lease before generation, release it at terminal state; default owned-model idle unload is five minutes. Never unload attached/external workloads automatically. Shut down idle owned processes on application exit; failure is visible. A scheduled task may start its approved local runtime when due while Lattice is open.

llmster is documented as a standalone daemon; its CLI daemon lifecycle and HTTP server are distinct. Its model-list semantics can change with just-in-time loading, so inventory must distinguish installed from loaded state explicitly. The adapter explore step records tested runtime/CLI versions and disables or accounts for automatic loading instead of assuming endpoint semantics. [Headless runtime](https://lmstudio.ai/docs/developer/core/headless), [daemon start](https://lmstudio.ai/docs/cli/daemon/daemon-up), [model management API](https://lmstudio.ai/docs/developer/rest).

MCP supports stdio and Streamable HTTP; choosing only stdio is a deliberate v1 scope reduction. Remote MCP/OAuth is post-v1, not a pretend connected UI. [MCP transport specification](https://modelcontextprotocol.io/specification/2025-11-25/basic/transports).

## Dependency graph

Numbers identify hard capability dependencies; release numbering is the default merge order. Every implementation starts from all already accepted specs.

```text
0.1 → 0.2 IPC → 0.3 security → 0.4 settings
                                │
                                └→ 0.5 discovery → 0.6 lifecycle → 0.7 models
                                                                    ↓
                                                               0.8 streaming
                                                                    ↓
                                                               0.9 history
                                       ┌────────────────────────────┼───────────────┐
                                       ↓                            ↓               ↓
                                 0.10 credentials             0.14 workspace     0.20 memory
                                       ↓                            ↓               │
                                 0.11 remote                  0.15 agent/tools      │
                                  /         \                 /     |       \       │
                           0.12 Anthropic  0.13 Gemini   0.16 write 0.18 skills 0.19 MCP
                                                              ↓                     │
                                                        0.17 terminal               │
                                       └──────── all required branches ──────────────┘
                                                                    ↓
                                                               0.21 Spaces
                                                                    ↓
                                                               0.22 tasks
                                                                    ↓
                                                               0.23 scheduler
                                                                    ↓
                                                         0.24 Kanban (optional)
                                                                    ↓
                                           0.25 resources → 0.26 security qualification
                                                                    ↓
                                            0.27 packaging → 0.28 first-use qualification
                                                                    ↓
                                                             1.0.0-rc.N → 1.0.0
```

The drawing groups edges for readability; the exact prerequisites are in each release scope. Additional edges: 0.15 needs 0.11 to prove portable runs, 0.19 needs 0.6 for owned-process cleanup, 0.20 needs 0.14 for workspace scoping. 0.21 joins providers, tools, skills, MCP and memory.

Anthropic/Gemini adapter work can proceed independently after 0.11. Skills, MCP and memory can proceed independently after their prerequisites; write and terminal remain sequential. Packaging research and support documentation can proceed earlier; release qualification cannot pass before the complete workflow exists. Parallel work means separate bounded branches/PRs with declared ownership, not several active minors handed to one implementing agent. No agents are delegated work by this document alone.

## Dependency admission

Never add a dependency because it will be useful later. Each minor's design records the concrete consumer/problem, an alternative (including existing code/no dependency), bundled/runtime memory and startup cost, maintenance evidence/version/license, security implications and removal path. Prefer build-only generators and Rust libraries over resident helper services. A user-selected MCP server may require Node/Python; this is an optional on-demand external process, never a persistent Lattice backend or mandatory install prerequisite. Model/runtime distribution rights must be checked before redistribution; v1 uses separately installed llmster and model files.

External documentation was consulted on 2026-09-06. These references support boundary decisions, not certification of a particular runtime/model combination. Recheck exact API/protocol versions in the relevant minor's explore stage.
