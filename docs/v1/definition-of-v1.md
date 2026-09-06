# Definition of Lattice 1.0

Status: planned release contract. Nothing here claims that the foundation already provides these capabilities. [Roadmap](../roadmap.md) defines delivery units; [architecture](architecture-sequence.md) defines ownership.

## Product acceptance journey

On a supported fresh desktop install, a user can complete this journey without a persistent Node/Bun service or a second agent application:

1. Open Lattice and see honest runtime/setup status. Follow the documented official external installation step if llmster is absent; Lattice does not bundle or silently install it. The first model is acquired through the runtime's documented process, with size/license visible in Lattice's setup guidance. Return to Lattice to detect and verify it.
2. Start an approved local runtime, list installed models, select/load a qualified model, stream a chat response, cancel it, unload the model and stop owned runtime processes. Missing runtime/model, occupied endpoint and insufficient resources have actionable states.
3. Reopen persisted conversations and continue them. Configure a remote OpenAI-compatible, Anthropic or Gemini provider through secure native credential entry. Select it for the next run while retaining conversation, workspace rules, skills and memory. Disclose that assembled context will leave the machine before the first remote send.
4. Select a workspace directory, review its access scope and use built-in read tools. Explicitly approve a conflict-checked file edit or an exact foreground command. Inspect tool outcomes and cancellations; no terminal action bypasses approval.
5. Create a portable `SKILL.md` folder using an ordinary editor, import it in Lattice, enable/disable/select it, and see its instructions contribute to context. Connect a trusted local stdio MCP server, discover tools, approve a tool call and disconnect it.
6. Create a Space that groups workspace, provider/model, selected skills, MCP references, memory and permissions. Switching Space changes resolved context; it cannot silently expand access or mutate a running execution.
7. Add, edit, delete and retrieve explicit workspace memories. Search persisted conversations and understand which context was included. No vector service or automatic memory learning is necessary.
8. Create a task, execute it manually, inspect its result, retry deliberately, and schedule a safe read-only/text-generation task for one future time. Observe scheduled/blocked/running/completed/failed/missed/interrupted status and run history. Closed/sleeping applications do not promise execution; missed runs require an explicit decision on resume.

The separate llmster/model setup is a bounded external prerequisite, not an invisible automatic installer. The useful workflow after setup stays in Lattice. If this setup path cannot be completed reliably by a new user, 0.28 fails.

## Fixed v1 scope decisions

| Concern       | Required at 1.0                                                                                                                                       | Excluded                                                                                                          |
| ------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------- |
| Providers     | OpenAI-compatible local/remote, Anthropic, Gemini; text streaming and tool-call contract certification                                                | Universal compatibility, multimodal/voice, twenty providers                                                       |
| Local runtime | One qualified llmster/CLI version range; start/stop/discover/list/load/unload/state                                                                   | Other lifecycle adapters, bundled inference engines, model marketplace/downloader                                 |
| Models        | At least one small Qwen profile and one small Gemma profile qualified for chat; at least one of the exact qualified models passes tool-call scenarios | Family-wide compatibility claims or a guarantee that every model can call tools                                   |
| Agent         | One active run globally, sequential tools, bounded steps/time/output/context, cancellation and inspected failures                                     | Parallel agents, autonomous planning frameworks, background learning                                              |
| Filesystem    | Workspace-scoped list/read and explicitly approved create/update with conflict checks                                                                 | Delete/move trees, unrestricted home access, automatic file watching/indexing                                     |
| Terminal      | Exact foreground executable/argv/cwd approval, bounded output/time, process-tree cancellation                                                         | Full terminal emulator/PTY, sandbox claims, unattended terminal                                                   |
| Skills        | Folder import/discovery/enable/select/context loading, portable human-readable files                                                                  | Marketplace, auto-running scripts, learning/installing dependencies                                               |
| MCP           | User-configured trusted local stdio processes, tool discovery/calls/status/disconnect                                                                 | Remote HTTP/OAuth, prompts/resources/sampling/roots protocols beyond necessary tool operation, registry ecosystem |
| Memory        | Conversations, workspace rules, explicit memories, simple lexical retrieval                                                                           | Embeddings, graph/vector infrastructure, automatic memory writes                                                  |
| Tasks         | Manual runs and one-shot schedules; explicit retry/failure/result visibility                                                                          | Cron/recurrence, wake/login daemon, execution while app is closed, unattended write/terminal/MCP                  |
| Kanban        | Optional projection of existing tasks if 0.24 passes                                                                                                  | A 1.0 blocker, dependencies/sprints/custom workflows                                                              |
| Data          | Local SQLite, backups/migrations/deletion documented; OS secure secret store                                                                          | Cloud sync, collaboration, enterprise tenancy                                                                     |

Qualification records must name exact model ID/revision, format, quantization, context/output settings, runtime version and test results. Select profiles during 0.7, certify during 0.15/0.25; model family names alone are not evidence. Profiles are engineering recommendations derived from measurements, not a benchmark or branding claim.

## Cross-platform release contract

- **Supported v1:** macOS on Apple Silicon. 0.27 must declare an exact tested minimum macOS version and an additional tested current version in release support docs, based on Tauri/WebView/runtime prerequisites at that time. Test clean install, native credential entry/keychain, real llmster/model workflow, sleep/resume, upgrades and uninstallation. Signed/notarized distributable and a verified download are required. Missing signing access is a release blocker, not permission to claim qualification.
- **Preview/source support:** Linux x86_64 and Windows x86_64. Before 1.0, CI must compile the Rust workspace and frontend and run deterministic native/core tests on both; scheduled/release jobs verify no-bundle native builds. Unsupported secure-input/runtime operations return explicit unavailable errors. No stable installer, real inference performance or full product support claim on these platforms until separately qualified. Platform-neutral domain APIs cannot assume POSIX paths, signals or macOS keychain details.
- Intel macOS, Linux ARM, Windows ARM, mobile and distribution through app stores are outside the 1.0 support promise. Publish the matrix rather than implying that `bundle.targets: all` means platform support.

## Definition of Done for every minor

- [ ] Exactly one release objective and accepted OpenSpec change; prerequisites are implemented/verified and the change's behavioral deltas include failure, cancellation and denial where applicable.
- [ ] Design defines module ownership, contracts, state transitions, bounded resources, dependency admission and migrations before implementation. No product decision needed by that scope remains an unspecified TODO.
- [ ] Acceptance checklist for that minor is backed by test names or a reproducible manual/native verification record, including negative cases. Fixtures validate a real boundary, not framework defaults.
- [ ] Required checks pass on the exact candidate commit. Existing tests are not weakened. A blocked required check blocks completion and is reported precisely.
- [ ] User-visible states include loading/empty/unavailable/error/recovery appropriate to the feature; keyboard interaction and accessible labels are covered for new UI.
- [ ] New native exposure is denied by default and enforced in Rust, secret handling/redaction is verified, and owned work cleans up after cancellation/exit.
- [ ] Persistence changes include forward migration and failure/backup behavior; old data is never silently reset. Version metadata, changelog and compatibility notes agree when cutting a release.
- [ ] Specs, relevant ADRs, architecture/support documentation and verification evidence are reconciled. Archive only completed behavior and keep the capability status honest.

Full baseline check: `pnpm check`, plus `pnpm build:web`, relevant Playwright and native build checks as required by the scope. `pnpm check` does not currently include E2E, native build or OpenSpec validation. Do not call it full desktop certification. Documentation-only changes use formatting, link/reference and scope consistency checks rather than new implementation tests.

## Performance gates

These are proposed engineering budgets, not observed results. Reference profiling uses Apple Silicon with approximately 16 GB unified memory; record exact hardware/OS/build/model configuration without making it product branding.

| Gate           | Release   | Measurement and pass condition                                                                                                                                                                                                |
| -------------- | --------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P0: foundation | 0.3       | Release build, no runtime/model/MCP, 5-minute settled idle and 3 launch/exit cycles. Record native/WebView aggregate physical memory and CPU; no unexpected child process. Establish idle baseline and measurement method.    |
| P1: runtime    | 0.6       | Compare no runtime, attached runtime, owned idle daemon/server; 10 start/stop cycles. Record process identities/ownership and idle memory; no orphan owned process or duplicate daemon.                                       |
| P2: model      | 0.7       | Measure cold/warm load, streamed use once 0.8 exists, unload and reload for each candidate profile. Separate unloaded runtime footprint, model increment and app growth; insufficient-memory handling is visible.             |
| P3: agent      | 0.15      | 20 deterministic read-tool runs with cancellation plus a qualified real-model smoke. Record IPC/event queue maxima, bounded transcript growth, latency and returned idle footprint; no accumulating tasks/processes.          |
| P4: integrated | 0.25      | Repeat P0–P3 with history, skills, MCP, Spaces and tasks, 60-minute mixed-use soak and sleep/resume. Verify budgets below and record p50/p95 over at least 3 runs.                                                            |
| P5: release    | 0.28 / RC | Repeat key idle/active/cleanup measurements on signed release build; compare to 0.25 on the same reference configuration. Any regression beyond thresholds must be fixed or the target explicitly revised before RC approval. |

Target aggregate non-model overhead: approximately 1 GiB working target, **2 GiB release ceiling** for the qualified default workflow, including Lattice native/WebView, unloaded inference infrastructure and a small qualified MCP process. Report model allocation/increment and total machine pressure separately; do not count shared unified-memory pages twice or subtract unreliable model estimates to hide overhead. If attribution cannot separate model bytes, report aggregate observed footprint plus a stated estimate/uncertainty and use the unloaded measurements for the overhead comparison.

Release gates additionally require settled no-work CPU below 1% of one logical core averaged over 5 minutes, zero orphan owned processes after 10 seconds of shutdown, and return within 10% or 50 MiB (whichever is larger) of settled non-model baseline after repeated cleanup. These are chosen thresholds to validate, not guarantees about arbitrary MCP servers or models. A failed measurement blocks the gate pending a documented fix/re-scope; it cannot be converted into a performance claim. P2's active-stream measurement is completed in 0.8, not faked in 0.7.

Default bounded execution contract: one run, one loaded model, at most 8 tool calls and a 5-minute run deadline; individual tools at most 30 seconds, terminal at most 60 seconds, tool output at most 256 KiB and admitted single-file text at most 1 MiB. Start with at most 3 simultaneously active MCP servers, 64 discovered tools per server and 1 MiB protocol messages. 0.8/0.15/0.19 designs must turn these targets into enforced limits and document lower provider/model context limits. Backpressure must pause or fail a stream visibly rather than grow buffers indefinitely.

## Testing and CI evolution

| Entry     | Cheap PR verification                                                                                                                    | Targeted integration / release evidence                                                                                                                                                                      |
| --------- | ---------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 0.2–0.3   | Existing TypeScript/Rust checks, generated DTO drift, malformed IPC/error cases, CSP config tests                                        | Tauri command dispatch/serialization harness plus native shell smoke; a function call alone is insufficient. Verify production CSP in real WebView. Record native manual steps if automation is unavailable. |
| 0.4       | SQLite temporary databases, migration/rollback/disk error fixtures                                                                       | Restart and safe handling of incompatible/corrupt data; no model needed.                                                                                                                                     |
| 0.5–0.8   | Fixture CLI/status responses, disposable process helper, loopback fixture HTTP streaming/cancel tests                                    | Opt-in real llmster smoke with preinstalled pinned model; record missing prerequisites as skipped, never passed. No automatic multi-GB download.                                                             |
| 0.9–0.13  | History migration, credential adapter contract, provider protocol fixtures including fragmented/malformed streams and auth/rate failures | Maintainer-triggered real provider smoke with bounded spend and redacted evidence. Secrets never available to untrusted PR jobs. No credential/network prerequisite for ordinary PR tests.                   |
| 0.14–0.19 | Temporary filesystem/symlink races, fake clock/provider, exact permission checks, deterministic loop, disposable stdio MCP fixture       | Platform process-tree cleanup, native approval UI, real qualified tool-call models and trusted MCP smoke. Add Linux/Windows core unit builds by 0.14.                                                        |
| 0.20–0.24 | Deterministic retrieval, Space isolation, task transition/claim tests, clock/timezone/restart fixtures                                   | Playwright exercises typed application fakes for UI states; separate native evidence verifies real IPC and persistence. No browser fake is labeled native E2E.                                               |
| 0.25–0.28 | Maintain focused suite and change-based jobs                                                                                             | Resource/security suites, three-platform no-bundle builds, macOS installer/upgrade/first-use tests, dependency/license audit and signed artifacts.                                                           |

Move cheap web and core checks to Linux when the job split is introduced; keep a macOS native job for changes affecting desktop/Rust/security. Before the split, retain the existing macOS job. Add Windows/Linux compile/core checks at 0.14 when filesystem/platform assumptions matter, with native build/packaging matrices on scheduled/release runs by 0.27. Keep relevant required jobs mandatory, not skipped via overly broad path filters.

Introduce pinned contract/OpenSpec validation in 0.2; dependency advisory/license checks on a schedule from 0.3 and on manifest/lockfile changes, becoming a release gate in 0.26. Real llmster tests begin as manual opt-in in 0.6/0.7, with a bounded scheduled runner only if a maintained preprovisioned host exists. External-service outages do not break every PR; they block the corresponding release certification when evidence is missing. Use cached immutable fixture/model assets and record their origin/license.

## Release strategy and final quality bar

Pre-1.0 minors each deliver one coherent capability in one branch/PR; patch versions repair it. Numbers are not dates and can exceed `0.9`. Each minor may change unstable internal contracts only with migration and compatibility notes. A prerequisite's bug is a focused fix, not authorization to implement the next release.

After 0.28, cut `1.0.0-rc.N` candidates with frozen product scope. RC changes are blocker fixes, documentation corrections and qualification reruns. Version/install/migration/security/resource failures block promotion. `1.0.0` uses the qualified candidate behavior and a fresh signed artifact verification; no feature is slipped into promotion. Do not create tags/releases as part of this planning work.

At promotion all required journey steps, performance gates, provider/tool conformance, permissions, secret redaction, migration/backup/restore, macOS packaging and meaningful CI checks must pass. Publish recoverable error guidance, tested support/model matrices and an upgrade path. No major architectural TODO may remain in these flows. Automatic updater is not required: documented manual replacement plus transactional schema migration and pre-upgrade backup is the v1 path. Refuse newer schemas on downgrade and offer restoring a compatible backup.

## Explicitly post-v1

Advanced learning/multi-agent/planning, Vertex/Wisp integrations, Agentix adoption without a present need, extra lifecycle runtimes/providers, advanced Kanban, remote MCP/OAuth, recurring/unattended powerful automation, remote execution, server mode, web client, cloud sync and collaboration. These are possibilities, not committed release scopes.
