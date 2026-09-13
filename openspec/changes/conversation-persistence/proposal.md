# Conversation Persistence

## Objective

Make chat conversations survive restart: create, list, reopen, and delete provider-independent conversations with ordered canonical messages and their generation outcomes, using Rust-owned SQLite storage, so 0.10+'s remote providers and 0.15's agent runs have a durable record to attach to instead of the process-memory-only transcript 0.8 shipped.

## Rationale

0.8 (`local-streaming-chat`, still an open, unarchived change) deliberately deferred persistence: its own design.md states "None. This is a deliberate non-goal (0.9 owns persistence); ... `active_chat_run` and per-run sequence counters are process-memory only and do not survive restart." Closing Lattice today discards every conversation. The roadmap's critical path (`docs/roadmap.md`, "Critical path and parallel opportunities") requires 0.9 before 0.10's credentials and 0.11's remote provider switching, both of which need a stable conversation identity to attach provider/model provenance to per generation, and before 0.15's agent runs, which persist step identity through "conversation storage" per the roadmap's own 0.15 boundaries.

0.4 (`persistent-application-settings`) already established the pattern this change extends: Rust-owned `rusqlite` (bundled) storage under the OS app-data directory, versioned `PRAGMA user_version` migrations with pre-migration backups, and typed IPC with no SQL/path authority given to Angular (`docs/adr/0009-use-rusqlite-for-local-settings.md`; `openspec/changes/persistent-application-settings/specs/local-storage/spec.md`'s already-generic "Lattice SHALL Version Local SQLite Schema" and "Lattice SHALL Keep Storage Authority In Rust" requirements apply unchanged here and are not re-specified). `crates/lattice-core/src/storage/mod.rs`'s own module doc comment names this exact moment: "A future domain (0.9 conversations, and later memory/tasks) should get its own sibling module and its own store type behind its own IPC surface, not another `impl SettingsStore` here."

## Dependencies

- 0.8 local streaming chat (`openspec/changes/local-streaming-chat/`): the canonical `ChatRole`/`ChatMessage`/`ChatRequest`/`ChatStreamEvent`/`ChatFinishReason` shapes this change persists against, and the `start_chat_stream`/`cancel_chat_stream` command pair this change wraps. Still open/unarchived; this change amends (not modifies-as-a-delta, since no permanent `chat-streaming` spec exists yet to validate a `MODIFIED` section against — confirmed by 0.7/0.8's own precedent of not writing deltas against still-open capabilities) `local-streaming-chat`'s own draft artifacts in place, narrowly, where `start_chat_stream`'s wire shape gains conversation identity. The neutral `ChatRequest`/`ChatStreamEvent` provider-port types themselves are unchanged.
- 0.4 persistent application settings (ADR 0009): the `SettingsStore`/`storage/` module structure, migration cascade, and backup policy this change's `ConversationStore` reuses.

## Scope

- New `conversations` domain (`crates/lattice-core/src/conversations/`): canonical `Conversation`, `ConversationSummary`, `Message`, `GenerationStatus`, app-generated UUID identity (never a vendor ID), and a bounded checkpoint policy for in-progress assistant messages.
- New `storage/conversations.rs` repository (`ConversationStore`, a sibling store type to `SettingsStore`, opened against the same `lattice.sqlite3` file) with schema version 5 (`conversations`, `messages` tables), reusing `storage/migrations.rs`'s existing versioned-cascade/backup machinery.
- `list_conversations` (paginated), `get_conversation` (metadata plus a paginated window of messages — this is "reopen"), and `delete_conversation` (transactional, cascades to messages) IPC commands.
- `start_chat_stream`'s Tauri-layer request gains `conversation_id: Option<String>` (`None` creates a new conversation with the request's messages; `Some` continues an existing one), and its response (`ChatRunHandle`) gains `conversation_id` so the frontend learns a newly created ID on first send. The underlying `ChatRequest`/`ChatStreamEvent` provider-port shapes and `run_chat_stream`/`local_openai` adapter are untouched.
- Bounded checkpointing of the streaming assistant message (batched by delta count and elapsed time, not per-token) plus an always-synchronous final write on the run's terminal event.
- Restart reconciliation: any message still `streaming` when `ConversationStore` next opens is marked `interrupted`, once, with no automatic retry.
- A minimal History page (list, open, delete) and Chat page updates (load an existing conversation, start a new one), reusing the existing per-feature Angular structure (`core/api`, `core/state`, `pages/<feature>`).

## Non-goals

- Memory extraction, Spaces, or any cross-conversation retrieval (0.20/0.21).
- Sync, export, or multi-device conversation access.
- Tool calls, tool-result blocks, or any agent loop (0.15).
- Automatic retry or resumption of an `interrupted` generation after restart.
- Conversation rename/editing UI; titles are derived once from the first user message and are not user-editable in this change.
- Any remote provider, credential, or additional network egress (0.10/0.11+); this change only persists the existing local-only path.
- Full-text search across conversations (0.20's lexical search is the intended future home).

## Impacted Capabilities

- `conversations` (new)
- `chat-streaming` (amended in place — still open/unarchived; see "Dependencies" above and this change's design.md for the exact wire-shape delta)

`local-storage`'s existing generic requirements (schema versioning/migration/backup, Rust-only storage authority) already cover this change's schema-5 migration and IPC design without new wording. `providers` and `application-api` are unchanged: no new vendor field, and the generic "Lattice SHALL Check Application IPC Contracts" requirement already covers the new commands without a new scenario.
