# Conversation Persistence Design

## Ownership

New domain area, sibling to `model_runtime` and `providers`, not an extension of either: `providers` keeps owning the neutral inference request/event contract and the local adapter (0.8, unchanged by this change); `conversations` owns conversation/message identity, ordering, generation status, and the checkpoint policy that decides when a streaming reply is written to disk. `lattice-core` keeps owning every canonical type and all storage logic; `lattice-desktop` (the Tauri shell) only wires IPC commands and composes the existing `run_chat_stream` orchestration with the new store, exactly as it already composes `run_chat_stream` with the model-lease check today. Angular owns only History/Chat presentation.

New modules:

- `crates/lattice-core/src/conversations/mod.rs` — canonical `Conversation`, `ConversationSummary`, `Message`, `GenerationStatus`, ID generation (`new_conversation_id`/`new_message_id`, reusing the `uuid` v4 dependency 0.8 already added), title derivation, and the checkpoint-policy constants/helper. Reuses `providers::completion::{ChatRole, ChatMessage}` — no duplicate role/message-shape type is introduced.
- `crates/lattice-core/src/storage/conversations.rs` — the `ConversationStore` repository, sibling to `storage/settings.rs`/`storage/runtime.rs`, per `storage/mod.rs`'s own doc comment (quoted in proposal.md).

## Storage architecture: one file, one migration authority, two store types

`DesktopState` already holds `settings: Mutex<SettingsStore>` as the sole persistence handle. This change adds `conversations: Mutex<ConversationStore>` as a **second, independent `rusqlite::Connection`** opened against the same `lattice.sqlite3` file (not a second file): one physical database keeps ADR 0009's single backup/migration authority intact, while `ConversationStore` is genuinely "its own store type" per `storage/mod.rs`'s mandate, rather than a growing `impl SettingsStore` block.

Two same-process connections to one SQLite file need two small, explicit additions to `storage/database.rs::open_connection`, applied to both stores:

- `PRAGMA busy_timeout = 5000` — SQLite's default is `0` (immediate `SQLITE_BUSY` on any lock collision). Settings writes and conversation writes are always disjoint tables in disjoint transactions and are rare relative to read traffic, but without a busy timeout a settings write that happens to land during a conversation-store checkpoint's brief write transaction (or vice versa) would surface as a hard error instead of a short, invisible wait. This is the only concurrency change this repository has needed since 0.4, because 0.4–0.8 never had two connections open at once.
- `PRAGMA foreign_keys = ON` — off by default in SQLite; required for `messages`'s `ON DELETE CASCADE` (see schema below) to actually fire.

Both `SettingsStore::open` and `ConversationStore::open` call the same `storage::migrations::migrate()`. Tauri's `.setup()` opens `SettingsStore` first, then `ConversationStore`, synchronously on one thread (no race): whichever opens first performs the real versioned migration (schema 4 → 5 adds `conversations`/`messages`); the second sees `schema_version == CURRENT_SCHEMA_VERSION` already and takes the existing lightweight read-validation branch. `CURRENT_SCHEMA_VERSION` moves from 4 to 5 in `storage/mod.rs`.

**Alternatives considered:**

- _A single shared `Arc<Mutex<Connection>>` behind both store types_ — avoids SQLite-level lock contention entirely, but forces every conversation checkpoint write to also serialize behind any settings read/write via one Rust mutex, and requires restructuring `SettingsStore`'s existing `Connection`-owning shape (a change to already-shipped 0.4 code) for a contention scenario (settings write vs. conversation write racing in the same instant) that is already rare and now bounded by `busy_timeout`. Rejected as a larger, riskier change for a smaller actual problem.
- _A second SQLite file_ (e.g. `conversations.sqlite3`) — keeps the two domains fully independent, but splits backup/migration into two authorities where ADR 0009 established one, and buys nothing: the domains already write to disjoint tables, so there is no data-isolation benefit, only operational duplication (two files to back up, two schema versions to reason about). Rejected.

## Schema (version 5)

```sql
CREATE TABLE conversations (
  id TEXT PRIMARY KEY,
  title TEXT NOT NULL,
  created_at_unix_seconds INTEGER NOT NULL,
  updated_at_unix_seconds INTEGER NOT NULL
);
CREATE INDEX idx_conversations_updated ON conversations(updated_at_unix_seconds DESC, id DESC);

CREATE TABLE messages (
  id TEXT PRIMARY KEY,
  conversation_id TEXT NOT NULL REFERENCES conversations(id) ON DELETE CASCADE,
  sequence INTEGER NOT NULL,
  role TEXT NOT NULL CHECK (role IN ('system', 'user', 'assistant')),
  text TEXT NOT NULL,
  status TEXT NOT NULL CHECK (status IN ('complete', 'streaming', 'cancelled', 'failed', 'interrupted')),
  provider_key TEXT,
  model_key TEXT,
  error_message TEXT,
  created_at_unix_seconds INTEGER NOT NULL,
  updated_at_unix_seconds INTEGER NOT NULL,
  UNIQUE (conversation_id, sequence)
);
CREATE INDEX idx_messages_conversation ON messages(conversation_id, sequence);
CREATE INDEX idx_messages_status ON messages(status);
```

`id` values are app-generated UUID v4 strings (`new_conversation_id`/`new_message_id`), never a provider/vendor identifier — the roadmap's explicit "vendor IDs are never primary keys" requirement, and consistent with 0.8's `run_id` (also app-generated, never persisted as one of these IDs: a `run_id` identifies one streaming attempt, not a stored message). `provider_key`/`model_key` are recorded per assistant message (not per conversation), because a conversation's provider/model can change between messages once 0.11+ adds remote switching — 0.9 always writes the local provider's fixed key, but the column exists at the message grain from the start so 0.11 does not need a schema migration to relocate it. User/system messages leave `provider_key`/`model_key`/`error_message` `NULL`.

Titles are derived once, at conversation creation, from the first user message's text (trimmed, truncated to a bounded length) and never recomputed — no rename UI exists in this change (non-goal).

## Contracts

```rust
// crates/lattice-core/src/conversations/mod.rs
pub enum GenerationStatus { Complete, Streaming, Cancelled, Failed, Interrupted }

pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sequence: u64,
    pub role: ChatRole,           // reused from providers::completion
    pub text: String,
    pub status: GenerationStatus,
    pub provider_key: Option<String>,
    pub model_key: Option<String>,
    pub error_message: Option<String>,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub updated_at_unix_seconds: u64,
    pub message_count: u64,
    pub last_status: Option<GenerationStatus>,
}
```

IPC (Tauri command / `wire.rs` names):

```rust
pub const LIST_CONVERSATIONS_COMMAND: &str = "list_conversations";
pub const GET_CONVERSATION_COMMAND: &str = "get_conversation";
pub const DELETE_CONVERSATION_COMMAND: &str = "delete_conversation";

pub struct ListConversationsRequest { pub limit: u32, pub before: Option<ConversationCursor> }
pub struct ConversationCursor { pub updated_at_unix_seconds: u64, pub id: String }
pub struct ListConversationsResponse { pub conversations: Vec<ConversationSummary>, pub next_before: Option<ConversationCursor> }

pub struct GetConversationRequest { pub conversation_id: String, pub limit: u32, pub before_sequence: Option<u64> }
pub struct ConversationDetail { pub conversation: Conversation, pub messages: Vec<Message>, pub has_more_before: bool }

pub struct DeleteConversationRequest { pub conversation_id: String }
```

`limit` is clamped server-side to a fixed maximum (50 for conversations, 100 for messages) regardless of what the caller requests — "bounded pages," not a caller-chosen unbounded query.

## Amendment to 0.8 (`local-streaming-chat`, still open/unarchived)

`start_chat_stream`'s Tauri-layer request and `ChatRunHandle` gain conversation identity; the neutral `ChatRequest`/`ChatStreamEvent` provider-port types in `providers::completion` are **not** changed — the client (Angular `ChatStore`) continues to compose and send the full message transcript on every call, exactly as 0.8 shipped it, so the provider-dispatch code path is untouched:

```rust
pub struct StartChatStreamRequest {
    pub conversation_id: Option<String>, // None starts a new conversation
    pub chat: ChatRequest,               // unchanged from 0.8
}

pub struct ChatRunHandle {
    pub run_id: String,
    pub conversation_id: String, // present even for conversation_id: None — the newly created ID
}
```

Rationale for not making Rust the sole source of truth for history (a smaller change than originally considered during exploration): the client already holds the authoritative in-flight transcript it is about to send to the provider (identical to 0.8's behavior); re-deriving that same transcript from storage inside Rust would only be needed to _replace_ the client's copy, which buys nothing here and would have required changing `ChatRequest` itself, reopening 0.8's already-shipped, tested provider contract. Instead, the Tauri command handler persists whatever new messages the client's request adds beyond what is already stored for `conversation_id` (see "Checkpointing and message persistence" below), keeping `providers::completion` completely untouched by this change.

`openspec/changes/local-streaming-chat/design.md`'s "Contracts" code block and `specs/chat-streaming/spec.md` are updated in place (not via a `MODIFIED` delta section — no permanent `chat-streaming` spec exists yet for OpenSpec's validator to diff against, the same reason 0.7/0.8 did not write deltas against each other's still-open capabilities) to show this superseded `ChatRunHandle`/command shape, with an inline note pointing here. This keeps 0.8's own draft artifacts truthful about what the command now actually does, rather than leaving them silently stale.

## Checkpointing and message persistence

In the Tauri command handler (`apps/desktop/src-tauri/src/lib.rs::start_chat_stream`), before spawning the streaming thread:

1. If `conversation_id` is `None`: create a new conversation (title derived from `request.chat.messages`'s last `user` message), and persist every message in `request.chat.messages` as sequence `0..N`.
2. If `conversation_id` is `Some(id)`: load the conversation (refuse with `AppError::conversation_not_found` if missing), read the currently stored message count for it, and persist only the _new_ trailing messages in `request.chat.messages` beyond that count (normally exactly one — the user's new message). If the request has _fewer_ messages than are already stored, refuse with `AppError::conversation_conflict` ("The conversation changed; reload it before sending.") rather than silently truncating stored history.
3. Insert one new assistant `Message` row with `status = streaming`, empty `text`, before the orchestrator thread starts, so `get_conversation` immediately reflects an in-progress reply if queried mid-stream.
4. Wrap `run_chat_stream`'s `on_event` closure (already passed to the IPC channel unchanged) with a persistence side-effect:
   - `Delta { text, .. }`: append to an in-memory accumulator; flush the accumulated text to the assistant message row when either **20 deltas** have accumulated since the last flush or **1000ms** have elapsed since the last flush, whichever comes first (`CHECKPOINT_DELTA_BATCH = 20`, `CHECKPOINT_MIN_INTERVAL = Duration::from_millis(1000)`, both in `conversations::mod`). This bounds SQLite write frequency without waiting for the full reply; it is a documented, accepted trade-off that a crash between checkpoints can lose up to one batch/interval's worth of trailing text — never more, and never partial-UTF-8 (a flush only ever writes complete accumulated text).
   - `Completed`/`Cancelled`/`Failed`: always flush synchronously regardless of the last checkpoint's timing, and finalize the row's `status` (`Complete`/`Cancelled`/`Failed`) and, for `Failed`, `error_message` (the same safe `AppError.message` already sent over the channel — no raw internal error is stored that isn't already user-visible).
5. Update the conversation's `updated_at_unix_seconds` once at step 1–2 and once at the final flush (step 4's terminal branch) — not on every checkpoint — since it only drives list ordering, not per-message freshness.

The checkpoint write itself is a short, independent transaction on `ConversationStore`'s own connection/mutex — never held across the `on_event` call's own blocking work, and never nested with the `settings` mutex.

## Restart reconciliation

`ConversationStore::open()` runs one statement immediately after `migrations::migrate()` succeeds (guarded so it only executes when the `messages` table exists, i.e. schema ≥ 5):

```sql
UPDATE messages SET status = 'interrupted', updated_at_unix_seconds = ?now
WHERE status = 'streaming';
```

This is the only reconciliation this change performs: it labels, once, any message that was mid-stream when the process last stopped (crash, force-quit, or the graceful-exit path if it happened to race the very last checkpoint). No resume, no automatic retry (non-goal) — matching 0.7's restart-reconciliation precedent in `storage/runtime.rs` (`ModelRuntimeStatus::with_current_file_state`), which recomputes truth from reality on open rather than trusting a stale flag.

## Pagination

- **Conversation list**: keyset pagination on `(updated_at_unix_seconds DESC, id DESC)` — most-recently-active conversation first. `ListConversationsRequest.before` names the last row of the previous page; `next_before` in the response is `None` once fewer than `limit` rows are returned.
- **Message history**: `get_conversation` returns the most recent `limit` messages by `sequence` (ascending order in the response, so the UI can render top-to-bottom without re-sorting) plus `has_more_before`; a second call with `before_sequence` set to the lowest `sequence` already loaded fetches older messages. This keeps an initial reopen of a long conversation bounded regardless of its total length.

## UI

- `apps/desktop/src/app/pages/history/` (new): list of `ConversationSummary` (title, relative updated time, message count, a status badge when `last_status` is `interrupted`/`failed`/`cancelled`), a "Load more" action for pagination, an "Open" action navigating to `/chat?conversationId=<id>`, and a "Delete" action with a confirmation step.
- `apps/desktop/src/app/pages/chat/`: gains a "New chat" action (clears `conversationId`/transcript in `ChatStore`) and, when navigated to with a `conversationId`, calls `get_conversation` to hydrate the transcript before allowing further sends. `ChatStore.sendMessage` continues to build the full `ChatRequest.messages` array itself (unchanged from 0.8) and now also threads `conversationId` through `startChatStream`, storing the `conversationId` returned by the first response.
- `core/api/conversations-fallback.ts` (new, browser-preview simulation, mirroring `chat-fallback.ts`'s and `model-slot-fallback.ts`'s existing pattern rather than an "unavailable" stub): an in-memory list/detail/delete simulation so the web-preview build exercises the same UI states as native.

## Dependency admission

No new dependency. `rusqlite` (bundled, already a workspace dependency since 0.4) and `uuid` v4 (already a `lattice-core` dependency since 0.8) cover everything this change needs; `PRAGMA busy_timeout`/`PRAGMA foreign_keys` are plain SQL executed through the existing `rusqlite::Connection`, not a new API surface.

## Verification

Testing tier follows the Definition of v1's persistence row (0.4's precedent): temporary-file `SettingsStore`-style fixtures for `ConversationStore` (create/list/pagination ordering, delete cascade, delete-while-active-run conflict, close-and-reopen restart persistence, streaming→interrupted reconciliation, v4→v5 migration against a hand-built old-schema fixture, failed-migration rollback). No new native/manual evidence category is introduced beyond what 0.4/0.8 already established (a real native GUI restart remains a documented pending item, exactly as 0.4/0.6/0.7/0.8 already record in their own verification docs) — recorded in `docs/verification/0.9-conversation-persistence.md`, not asserted here.
