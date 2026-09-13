//! The 0.9 conversations repository: a sibling store type to
//! [`super::SettingsStore`], not another `impl SettingsStore` block, per
//! `storage/mod.rs`'s own doc comment. `ConversationStore` opens a second,
//! independent `rusqlite::Connection` to the *same* `lattice.sqlite3` file
//! `SettingsStore` already owns — one physical database keeps ADR 0009's
//! single backup/migration authority, while this remains genuinely its own
//! store type behind its own IPC surface. Schema evolution (the `conversations`/
//! `messages` tables) lives in `migrations::create_v5_schema`, matching every
//! other domain's `create_vN_schema` convention; this file only implements
//! what a current-schema connection is used for.
//!
//! Two same-process connections to one file is why `database::open_connection`
//! now sets `PRAGMA busy_timeout`/`PRAGMA foreign_keys` on every connection
//! (see that module's doc comments) rather than assuming a single exclusive
//! writer, as 0.4–0.8 could.

use super::{database, migrations};
use crate::conversations::{
    conversations_page_size, derive_conversation_title, messages_page_size, new_conversation_id,
    new_message_id, unix_timestamp_now, Conversation, ConversationCursor, ConversationDetail,
    ConversationSummary, DeleteConversationRequest, GenerationStatus, GetConversationRequest,
    ListConversationsRequest, ListConversationsResponse, Message, LOCAL_PROVIDER_KEY,
};
use crate::providers::{ChatMessage, ChatRole};
use crate::AppError;
use rusqlite::{params, Connection, OptionalExtension, Row};
use std::path::Path;

pub struct ConversationStore {
    conn: Connection,
}

impl ConversationStore {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref();
        let mut conn = database::open_connection(path)?;
        migrations::migrate(&mut conn, Some(path))?;
        reconcile_interrupted_messages(&conn)?;
        Ok(Self { conn })
    }

    #[cfg(test)]
    fn open_in_memory() -> Result<Self, AppError> {
        let mut conn = Connection::open_in_memory().map_err(|_| {
            AppError::storage_unavailable("Lattice could not open local conversation storage.")
        })?;
        database::apply_connection_pragmas(&conn)?;
        migrations::migrate(&mut conn, None)?;
        reconcile_interrupted_messages(&conn)?;
        Ok(Self { conn })
    }

    /// Creates a new conversation (when `conversation_id` is `None`) or
    /// appends the request's new trailing messages to an existing one, and
    /// returns the resolved conversation ID. The client is assumed to send
    /// its full local transcript on every call, exactly as 0.8 already
    /// does; only messages beyond what is already stored are persisted, so
    /// no message is ever written twice. See design.md's "Amendment to
    /// 0.8" for why the client, not Rust, remains the source of the
    /// transcript sent to the provider.
    pub fn begin_or_continue(
        &mut self,
        conversation_id: Option<&str>,
        messages: &[ChatMessage],
    ) -> Result<String, AppError> {
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not access conversation storage.")
        })?;
        let now = unix_timestamp_now();

        let resolved_id = match conversation_id {
            None => {
                let id = new_conversation_id();
                let title_source = messages
                    .iter()
                    .rev()
                    .find(|message| message.role == ChatRole::User)
                    .map(|message| message.text.as_str())
                    .unwrap_or("");
                let title = derive_conversation_title(title_source);

                tx.execute(
                    "INSERT INTO conversations (id, title, created_at_unix_seconds, updated_at_unix_seconds)
                     VALUES (?1, ?2, ?3, ?3)",
                    params![id, title, i64_or_zero(now)],
                )
                .map_err(|_| AppError::storage_unavailable("Lattice could not create the conversation."))?;
                insert_messages(&tx, &id, messages, 0, now)?;
                id
            }
            Some(id) => {
                let exists: bool = tx
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM conversations WHERE id = ?1)",
                        params![id],
                        |row| row.get(0),
                    )
                    .map_err(|_| {
                        AppError::storage_unavailable("Lattice could not read the conversation.")
                    })?;
                if !exists {
                    return Err(AppError::conversation_not_found(
                        "That conversation no longer exists.",
                    ));
                }

                let existing_count: i64 = tx
                    .query_row(
                        "SELECT COUNT(*) FROM messages WHERE conversation_id = ?1",
                        params![id],
                        |row| row.get(0),
                    )
                    .map_err(|_| {
                        AppError::storage_unavailable("Lattice could not read the conversation.")
                    })?;
                let existing_count = u64::try_from(existing_count).unwrap_or(0);

                if (messages.len() as u64) < existing_count {
                    return Err(AppError::conversation_conflict(
                        "The conversation changed; reload it before sending.",
                    ));
                }

                let new_messages = &messages[existing_count as usize..];
                if !new_messages.is_empty() {
                    insert_messages(&tx, id, new_messages, existing_count, now)?;
                    tx.execute(
                        "UPDATE conversations SET updated_at_unix_seconds = ?1 WHERE id = ?2",
                        params![i64_or_zero(now), id],
                    )
                    .map_err(|_| {
                        AppError::storage_unavailable("Lattice could not update the conversation.")
                    })?;
                }
                id.to_string()
            }
        };

        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save the conversation.")
        })?;
        Ok(resolved_id)
    }

    /// Inserts a new empty, `streaming` assistant message and returns its
    /// ID, before the orchestrator thread produces its first event — so
    /// `get` immediately reflects an in-progress reply if queried mid-stream.
    pub fn start_assistant_message(
        &mut self,
        conversation_id: &str,
        model_key: &str,
    ) -> Result<String, AppError> {
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not access conversation storage.")
        })?;
        let next_sequence: i64 = tx
            .query_row(
                "SELECT COALESCE(MAX(sequence), -1) + 1 FROM messages WHERE conversation_id = ?1",
                params![conversation_id],
                |row| row.get(0),
            )
            .map_err(|_| {
                AppError::storage_unavailable("Lattice could not read the conversation.")
            })?;

        let id = new_message_id();
        let now = i64_or_zero(unix_timestamp_now());
        tx.execute(
            "INSERT INTO messages (
                id, conversation_id, sequence, role, text, status,
                provider_key, model_key, error_message,
                created_at_unix_seconds, updated_at_unix_seconds
             ) VALUES (?1, ?2, ?3, 'assistant', '', 'streaming', ?4, ?5, NULL, ?6, ?6)",
            params![
                id,
                conversation_id,
                next_sequence,
                LOCAL_PROVIDER_KEY,
                model_key,
                now
            ],
        )
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not start the assistant message.")
        })?;
        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not start the assistant message.")
        })?;
        Ok(id)
    }

    /// Bounded checkpoint write: overwrites the streaming message's
    /// accumulated text. Never changes `status` — only `finalize_assistant_message`
    /// does, exactly once, on the run's terminal event. A message that is
    /// no longer `streaming` (already finalized, or reconciled to
    /// `interrupted` by a since-elapsed restart) silently accepts no write,
    /// since a checkpoint arriving after finalization would only be a
    /// harmless race with the terminal event, never a correctness issue to
    /// surface as an error to the caller.
    pub fn checkpoint_assistant_message(
        &self,
        message_id: &str,
        text: &str,
    ) -> Result<(), AppError> {
        let now = i64_or_zero(unix_timestamp_now());
        self.conn
            .execute(
                "UPDATE messages SET text = ?1, updated_at_unix_seconds = ?2
                 WHERE id = ?3 AND status = 'streaming'",
                params![text, now, message_id],
            )
            .map_err(|_| {
                AppError::storage_unavailable("Lattice could not save the streaming reply.")
            })?;
        Ok(())
    }

    /// Always-synchronous final write for a message's run: writes the
    /// complete accumulated text, the terminal `status`, and (for `Failed`)
    /// the same safe error message already sent over the IPC channel.
    /// Also bumps the owning conversation's `updated_at_unix_seconds`,
    /// exactly once per run, so conversation list ordering reflects
    /// activity without a write per checkpoint.
    pub fn finalize_assistant_message(
        &mut self,
        message_id: &str,
        text: &str,
        status: GenerationStatus,
        error_message: Option<&str>,
    ) -> Result<(), AppError> {
        let tx = self.conn.transaction().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save the streaming reply.")
        })?;
        let now = i64_or_zero(unix_timestamp_now());

        tx.execute(
            "UPDATE messages
             SET text = ?1, status = ?2, error_message = ?3, updated_at_unix_seconds = ?4
             WHERE id = ?5",
            params![
                text,
                status.as_storage_value(),
                error_message,
                now,
                message_id
            ],
        )
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not save the streaming reply.")
        })?;

        tx.execute(
            "UPDATE conversations SET updated_at_unix_seconds = ?1
             WHERE id = (SELECT conversation_id FROM messages WHERE id = ?2)",
            params![now, message_id],
        )
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not save the streaming reply.")
        })?;

        tx.commit().map_err(|_| {
            AppError::storage_unavailable("Lattice could not save the streaming reply.")
        })?;
        Ok(())
    }

    /// Lists conversations most-recently-updated first, keyset-paginated on
    /// `(updated_at_unix_seconds, id)` descending. Bounded server-side
    /// regardless of the caller's requested `limit`.
    pub fn list(
        &self,
        request: ListConversationsRequest,
    ) -> Result<ListConversationsResponse, AppError> {
        let limit = conversations_page_size(request.limit);
        let fetch_limit = i64::from(limit) + 1;
        let (before_updated, before_id): (Option<i64>, Option<String>) = match request.before {
            Some(cursor) => (
                Some(i64::try_from(cursor.updated_at_unix_seconds).unwrap_or(i64::MAX)),
                Some(cursor.id),
            ),
            None => (None, None),
        };

        let mut statement = self
            .conn
            .prepare(
                "SELECT c.id, c.title, c.updated_at_unix_seconds,
                        (SELECT COUNT(*) FROM messages m WHERE m.conversation_id = c.id) AS message_count,
                        (SELECT m2.status FROM messages m2 WHERE m2.conversation_id = c.id
                            ORDER BY m2.sequence DESC LIMIT 1) AS last_status
                 FROM conversations c
                 WHERE (?1 IS NULL)
                    OR (c.updated_at_unix_seconds < ?1)
                    OR (c.updated_at_unix_seconds = ?1 AND c.id < ?2)
                 ORDER BY c.updated_at_unix_seconds DESC, c.id DESC
                 LIMIT ?3",
            )
            .map_err(|_| AppError::storage_unavailable("Lattice could not list conversations."))?;

        let rows = statement
            .query_map(params![before_updated, before_id, fetch_limit], |row| {
                Ok(RawSummaryRow {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    updated_at_unix_seconds: row.get(2)?,
                    message_count: row.get(3)?,
                    last_status: row.get(4)?,
                })
            })
            .map_err(|_| AppError::storage_unavailable("Lattice could not list conversations."))?;

        let mut summaries = Vec::new();
        for row in rows {
            summaries.push(row.map_err(|_| {
                AppError::storage_unavailable("Lattice could not list conversations.")
            })?);
        }

        let has_more = summaries.len() > limit as usize;
        summaries.truncate(limit as usize);

        let next_before =
            has_more
                .then(|| summaries.last())
                .flatten()
                .map(|row| ConversationCursor {
                    updated_at_unix_seconds: nonneg_u64(row.updated_at_unix_seconds),
                    id: row.id.clone(),
                });

        let conversations = summaries
            .into_iter()
            .map(|row| ConversationSummary {
                id: row.id,
                title: row.title,
                updated_at_unix_seconds: nonneg_u64(row.updated_at_unix_seconds),
                message_count: nonneg_u64(row.message_count),
                last_status: row
                    .last_status
                    .as_deref()
                    .and_then(GenerationStatus::from_storage_value),
            })
            .collect();

        Ok(ListConversationsResponse {
            conversations,
            next_before,
        })
    }

    /// Reopens a conversation: its metadata plus a bounded, paginated
    /// window of its messages in ascending sequence order (most recent
    /// page by default; `before_sequence` walks further back).
    pub fn get(&self, request: GetConversationRequest) -> Result<ConversationDetail, AppError> {
        let conversation = self
            .conn
            .query_row(
                "SELECT id, title, created_at_unix_seconds, updated_at_unix_seconds
                 FROM conversations WHERE id = ?1",
                params![request.conversation_id],
                |row| {
                    Ok(Conversation {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        created_at_unix_seconds: nonneg_u64(row.get(2)?),
                        updated_at_unix_seconds: nonneg_u64(row.get(3)?),
                    })
                },
            )
            .optional()
            .map_err(|_| AppError::storage_unavailable("Lattice could not read the conversation."))?
            .ok_or_else(|| {
                AppError::conversation_not_found("That conversation no longer exists.")
            })?;

        let limit = messages_page_size(request.limit);
        let fetch_limit = i64::from(limit) + 1;
        let before_sequence = request
            .before_sequence
            .map(|sequence| i64::try_from(sequence).unwrap_or(i64::MAX));

        let mut statement = self
            .conn
            .prepare(
                "SELECT id, conversation_id, sequence, role, text, status,
                        provider_key, model_key, error_message,
                        created_at_unix_seconds, updated_at_unix_seconds
                 FROM messages
                 WHERE conversation_id = ?1 AND (?2 IS NULL OR sequence < ?2)
                 ORDER BY sequence DESC
                 LIMIT ?3",
            )
            .map_err(|_| {
                AppError::storage_unavailable("Lattice could not read the conversation.")
            })?;

        let rows = statement
            .query_map(
                params![request.conversation_id, before_sequence, fetch_limit],
                raw_message_row,
            )
            .map_err(|_| {
                AppError::storage_unavailable("Lattice could not read the conversation.")
            })?;

        let mut raw_messages = Vec::new();
        for row in rows {
            raw_messages.push(row.map_err(|_| {
                AppError::storage_unavailable("Lattice could not read the conversation.")
            })?);
        }

        let has_more_before = raw_messages.len() > limit as usize;
        raw_messages.truncate(limit as usize);
        raw_messages.reverse();

        let mut messages = Vec::with_capacity(raw_messages.len());
        for raw in raw_messages {
            messages.push(raw.into_message()?);
        }

        Ok(ConversationDetail {
            conversation,
            messages,
            has_more_before,
        })
    }

    /// Deletes a conversation and, via `ON DELETE CASCADE`, every one of
    /// its messages. Refusing a delete while a run is active against this
    /// conversation is enforced by the Tauri command layer (which alone
    /// knows about the in-process active-run slot), not here.
    pub fn delete(&mut self, request: DeleteConversationRequest) -> Result<(), AppError> {
        let changed = self
            .conn
            .execute(
                "DELETE FROM conversations WHERE id = ?1",
                params![request.conversation_id],
            )
            .map_err(|_| {
                AppError::storage_unavailable("Lattice could not delete the conversation.")
            })?;

        if changed == 0 {
            return Err(AppError::conversation_not_found(
                "That conversation no longer exists.",
            ));
        }
        Ok(())
    }
}

fn insert_messages(
    tx: &rusqlite::Transaction<'_>,
    conversation_id: &str,
    messages: &[ChatMessage],
    start_sequence: u64,
    now: u64,
) -> Result<(), AppError> {
    let now = i64_or_zero(now);
    for (offset, message) in messages.iter().enumerate() {
        let sequence = i64_or_zero(start_sequence + offset as u64);
        tx.execute(
            "INSERT INTO messages (
                id, conversation_id, sequence, role, text, status,
                provider_key, model_key, error_message,
                created_at_unix_seconds, updated_at_unix_seconds
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'complete', NULL, NULL, NULL, ?6, ?6)",
            params![
                new_message_id(),
                conversation_id,
                sequence,
                role_storage_value(message.role),
                message.text,
                now
            ],
        )
        .map_err(|_| {
            AppError::storage_unavailable("Lattice could not save the conversation message.")
        })?;
    }
    Ok(())
}

/// Runs once per `ConversationStore::open`: any message still `streaming`
/// when storage was last closed (crash, force-quit) is labeled
/// `interrupted`. No resume, no automatic retry — see the conversations
/// spec's restart-reconciliation requirement.
fn reconcile_interrupted_messages(conn: &Connection) -> Result<(), AppError> {
    let now = i64_or_zero(unix_timestamp_now());
    conn.execute(
        "UPDATE messages SET status = 'interrupted', updated_at_unix_seconds = ?1
         WHERE status = 'streaming'",
        params![now],
    )
    .map_err(|_| {
        AppError::storage_unavailable("Lattice could not reconcile local conversations.")
    })?;
    Ok(())
}

/// Read-validation for `migrations::migrate`'s "schema already current"
/// branch, mirroring `settings::read_settings`/`runtime::read_model_runtime_status`.
pub(super) fn read_conversations_sanity(conn: &Connection) -> Result<(), AppError> {
    conn.query_row("SELECT COUNT(*) FROM conversations", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|_| AppError::storage_unavailable("Lattice could not read local conversations."))?;
    conn.query_row("SELECT COUNT(*) FROM messages", [], |row| {
        row.get::<_, i64>(0)
    })
    .map_err(|_| AppError::storage_unavailable("Lattice could not read local conversations."))?;
    Ok(())
}

fn role_storage_value(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

fn role_from_storage_value(value: &str) -> Option<ChatRole> {
    match value {
        "system" => Some(ChatRole::System),
        "user" => Some(ChatRole::User),
        "assistant" => Some(ChatRole::Assistant),
        _ => None,
    }
}

fn nonneg_u64(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

fn i64_or_zero(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(0)
}

struct RawSummaryRow {
    id: String,
    title: String,
    updated_at_unix_seconds: i64,
    message_count: i64,
    last_status: Option<String>,
}

struct RawMessageRow {
    id: String,
    conversation_id: String,
    sequence: i64,
    role: String,
    text: String,
    status: String,
    provider_key: Option<String>,
    model_key: Option<String>,
    error_message: Option<String>,
    created_at_unix_seconds: i64,
    updated_at_unix_seconds: i64,
}

impl RawMessageRow {
    /// Converts raw, already-CHECK-constrained SQLite columns into the
    /// typed `Message`, mirroring `settings::read_settings`'s pattern of
    /// validating string-encoded enums outside the row-mapping closure. A
    /// conversion failure here means the on-disk value no longer matches
    /// what this binary's CHECK constraints allow (foreign/corrupted
    /// storage), not a caller mistake — reported the same safe way
    /// `read_settings` reports any other unreadable row.
    fn into_message(self) -> Result<Message, AppError> {
        let role = role_from_storage_value(&self.role).ok_or_else(|| {
            AppError::storage_unavailable("Lattice could not read a conversation message.")
        })?;
        let status = GenerationStatus::from_storage_value(&self.status).ok_or_else(|| {
            AppError::storage_unavailable("Lattice could not read a conversation message.")
        })?;

        Ok(Message {
            id: self.id,
            conversation_id: self.conversation_id,
            sequence: nonneg_u64(self.sequence),
            role,
            text: self.text,
            status,
            provider_key: self.provider_key,
            model_key: self.model_key,
            error_message: self.error_message,
            created_at_unix_seconds: nonneg_u64(self.created_at_unix_seconds),
            updated_at_unix_seconds: nonneg_u64(self.updated_at_unix_seconds),
        })
    }
}

fn raw_message_row(row: &Row<'_>) -> rusqlite::Result<RawMessageRow> {
    Ok(RawMessageRow {
        id: row.get(0)?,
        conversation_id: row.get(1)?,
        sequence: row.get(2)?,
        role: row.get(3)?,
        text: row.get(4)?,
        status: row.get(5)?,
        provider_key: row.get(6)?,
        model_key: row.get(7)?,
        error_message: row.get(8)?,
        created_at_unix_seconds: row.get(9)?,
        updated_at_unix_seconds: row.get(10)?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{error::Error, thread, time::Duration};
    use tempfile::tempdir;

    fn user_message(text: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::User,
            text: text.to_string(),
        }
    }

    fn assistant_message(text: &str) -> ChatMessage {
        ChatMessage {
            role: ChatRole::Assistant,
            text: text.to_string(),
        }
    }

    #[test]
    fn creates_a_conversation_on_first_message() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let id = store.begin_or_continue(None, &[user_message("Hello there, Lattice")])?;

        let detail = store.get(GetConversationRequest {
            conversation_id: id.clone(),
            limit: None,
            before_sequence: None,
        })?;

        assert_eq!(detail.conversation.id, id);
        assert_eq!(detail.conversation.title, "Hello there, Lattice");
        assert_eq!(detail.messages.len(), 1);
        assert_eq!(detail.messages[0].role, ChatRole::User);
        assert_eq!(detail.messages[0].sequence, 0);
        assert_eq!(detail.messages[0].status, GenerationStatus::Complete);
        Ok(())
    }

    #[test]
    fn continuing_appends_only_new_trailing_messages() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let id = store.begin_or_continue(None, &[user_message("first")])?;
        store.begin_or_continue(
            Some(&id),
            &[
                user_message("first"),
                assistant_message("reply"),
                user_message("second"),
            ],
        )?;

        let detail = store.get(GetConversationRequest {
            conversation_id: id,
            limit: None,
            before_sequence: None,
        })?;

        assert_eq!(detail.messages.len(), 3);
        assert_eq!(detail.messages[1].text, "reply");
        assert_eq!(detail.messages[2].text, "second");
        assert_eq!(detail.messages[2].sequence, 2);
        Ok(())
    }

    #[test]
    fn continuing_unknown_conversation_is_not_found() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let error = store
            .begin_or_continue(Some("missing"), &[user_message("hi")])
            .err()
            .ok_or("expected not-found error")?;
        assert_eq!(error.code, "conversation.not_found");
        Ok(())
    }

    #[test]
    fn continuing_with_stale_shorter_history_is_a_conflict() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let id =
            store.begin_or_continue(None, &[user_message("first"), assistant_message("reply")])?;

        let error = store
            .begin_or_continue(Some(&id), &[user_message("only one")])
            .err()
            .ok_or("expected conflict error")?;
        assert_eq!(error.code, "conversation.conflict");

        let detail = store.get(GetConversationRequest {
            conversation_id: id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages.len(), 2);
        Ok(())
    }

    #[test]
    fn streaming_message_checkpoints_then_finalizes() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let conversation_id = store.begin_or_continue(None, &[user_message("hi")])?;
        let message_id = store.start_assistant_message(&conversation_id, "qwen-small")?;

        let mid_stream = store.get(GetConversationRequest {
            conversation_id: conversation_id.clone(),
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(mid_stream.messages[1].status, GenerationStatus::Streaming);
        assert_eq!(mid_stream.messages[1].text, "");

        store.checkpoint_assistant_message(&message_id, "Hello")?;
        store.finalize_assistant_message(
            &message_id,
            "Hello, world!",
            GenerationStatus::Complete,
            None,
        )?;

        let finished = store.get(GetConversationRequest {
            conversation_id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(finished.messages[1].status, GenerationStatus::Complete);
        assert_eq!(finished.messages[1].text, "Hello, world!");
        assert_eq!(
            finished.messages[1].provider_key.as_deref(),
            Some(LOCAL_PROVIDER_KEY)
        );
        assert_eq!(
            finished.messages[1].model_key.as_deref(),
            Some("qwen-small")
        );
        Ok(())
    }

    #[test]
    fn finalize_records_a_failed_generation_with_its_error_message() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let conversation_id = store.begin_or_continue(None, &[user_message("hi")])?;
        let message_id = store.start_assistant_message(&conversation_id, "qwen-small")?;

        store.finalize_assistant_message(
            &message_id,
            "partial",
            GenerationStatus::Failed,
            Some("The local model runtime stopped responding unexpectedly."),
        )?;

        let detail = store.get(GetConversationRequest {
            conversation_id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages[1].status, GenerationStatus::Failed);
        assert_eq!(
            detail.messages[1].error_message.as_deref(),
            Some("The local model runtime stopped responding unexpectedly.")
        );
        Ok(())
    }

    #[test]
    fn checkpoint_after_finalize_does_not_resurrect_the_message() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let conversation_id = store.begin_or_continue(None, &[user_message("hi")])?;
        let message_id = store.start_assistant_message(&conversation_id, "qwen-small")?;
        store.finalize_assistant_message(&message_id, "done", GenerationStatus::Complete, None)?;

        store.checkpoint_assistant_message(&message_id, "late, abandoned text")?;

        let detail = store.get(GetConversationRequest {
            conversation_id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages[1].status, GenerationStatus::Complete);
        assert_eq!(detail.messages[1].text, "done");
        Ok(())
    }

    #[test]
    fn reopens_a_conversation_after_restart() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conversation_id = {
            let mut store = ConversationStore::open(&path)?;
            let id = store.begin_or_continue(None, &[user_message("Persist me")])?;
            let message_id = store.start_assistant_message(&id, "qwen-small")?;
            store.finalize_assistant_message(
                &message_id,
                "Sure thing.",
                GenerationStatus::Complete,
                None,
            )?;
            id
        };

        let reopened = ConversationStore::open(&path)?;
        let detail = reopened.get(GetConversationRequest {
            conversation_id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages.len(), 2);
        assert_eq!(detail.messages[1].text, "Sure thing.");
        Ok(())
    }

    #[test]
    fn a_message_left_streaming_becomes_interrupted_on_reopen() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conversation_id = {
            let mut store = ConversationStore::open(&path)?;
            let id = store.begin_or_continue(None, &[user_message("hi")])?;
            let message_id = store.start_assistant_message(&id, "qwen-small")?;
            store.checkpoint_assistant_message(&message_id, "partway through")?;
            id
            // `store` is dropped here without ever finalizing, simulating a crash.
        };

        let reopened = ConversationStore::open(&path)?;
        let detail = reopened.get(GetConversationRequest {
            conversation_id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages[1].status, GenerationStatus::Interrupted);
        assert_eq!(detail.messages[1].text, "partway through");
        Ok(())
    }

    #[test]
    fn a_cleanly_terminated_message_is_not_reclassified_on_reopen() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conversation_id = {
            let mut store = ConversationStore::open(&path)?;
            let id = store.begin_or_continue(None, &[user_message("hi")])?;
            let message_id = store.start_assistant_message(&id, "qwen-small")?;
            store.finalize_assistant_message(
                &message_id,
                "done",
                GenerationStatus::Cancelled,
                None,
            )?;
            id
        };

        let reopened = ConversationStore::open(&path)?;
        let detail = reopened.get(GetConversationRequest {
            conversation_id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages[1].status, GenerationStatus::Cancelled);
        Ok(())
    }

    #[test]
    fn deleting_a_conversation_removes_its_messages() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let keep = store.begin_or_continue(None, &[user_message("keep me")])?;
        let delete_me = store.begin_or_continue(None, &[user_message("delete me")])?;

        store.delete(DeleteConversationRequest {
            conversation_id: delete_me.clone(),
        })?;

        let error = store
            .get(GetConversationRequest {
                conversation_id: delete_me,
                limit: None,
                before_sequence: None,
            })
            .err()
            .ok_or("expected not-found after delete")?;
        assert_eq!(error.code, "conversation.not_found");

        let still_there = store.get(GetConversationRequest {
            conversation_id: keep,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(still_there.messages.len(), 1);
        Ok(())
    }

    #[test]
    fn deleting_an_unknown_conversation_is_not_found() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let error = store
            .delete(DeleteConversationRequest {
                conversation_id: "missing".to_string(),
            })
            .err()
            .ok_or("expected not-found error")?;
        assert_eq!(error.code, "conversation.not_found");
        Ok(())
    }

    #[test]
    fn lists_conversations_most_recently_updated_first_and_paginates() -> Result<(), Box<dyn Error>>
    {
        let mut store = ConversationStore::open_in_memory()?;
        let mut ids = Vec::new();
        for index in 0..5 {
            ids.push(
                store.begin_or_continue(None, &[user_message(&format!("conversation {index}"))])?,
            );
            // Ensure distinct `updated_at_unix_seconds` so ordering is unambiguous.
            thread::sleep(Duration::from_millis(1100));
        }

        let first_page = store.list(ListConversationsRequest {
            limit: Some(2),
            before: None,
        })?;
        assert_eq!(first_page.conversations.len(), 2);
        assert_eq!(first_page.conversations[0].id, ids[4]);
        assert_eq!(first_page.conversations[1].id, ids[3]);
        let cursor = first_page
            .next_before
            .ok_or("expected a next page cursor")?;

        let second_page = store.list(ListConversationsRequest {
            limit: Some(2),
            before: Some(cursor),
        })?;
        assert_eq!(second_page.conversations.len(), 2);
        assert_eq!(second_page.conversations[0].id, ids[2]);
        assert_eq!(second_page.conversations[1].id, ids[1]);
        assert!(second_page.next_before.is_some());
        Ok(())
    }

    #[test]
    fn list_reports_message_count_and_last_status() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let id = store.begin_or_continue(None, &[user_message("hi")])?;
        let message_id = store.start_assistant_message(&id, "qwen-small")?;
        store.finalize_assistant_message(&message_id, "hello", GenerationStatus::Complete, None)?;

        let page = store.list(ListConversationsRequest {
            limit: None,
            before: None,
        })?;
        assert_eq!(page.conversations[0].message_count, 2);
        assert_eq!(
            page.conversations[0].last_status,
            Some(GenerationStatus::Complete)
        );
        Ok(())
    }

    #[test]
    fn get_paginates_long_message_history() -> Result<(), Box<dyn Error>> {
        let mut store = ConversationStore::open_in_memory()?;
        let id = store.begin_or_continue(None, &[user_message("seed")])?;
        for index in 0..9 {
            store.begin_or_continue(
                Some(&id),
                &(0..=index + 1)
                    .map(|n| user_message(&format!("message {n}")))
                    .collect::<Vec<_>>(),
            )?;
        }

        let latest_page = store.get(GetConversationRequest {
            conversation_id: id.clone(),
            limit: Some(4),
            before_sequence: None,
        })?;
        assert_eq!(latest_page.messages.len(), 4);
        assert!(latest_page.has_more_before);
        assert_eq!(latest_page.messages[0].sequence, 6);
        assert_eq!(latest_page.messages[3].sequence, 9);

        let older_page = store.get(GetConversationRequest {
            conversation_id: id,
            limit: Some(4),
            before_sequence: Some(latest_page.messages[0].sequence),
        })?;
        assert_eq!(older_page.messages.len(), 4);
        assert_eq!(older_page.messages[0].sequence, 2);
        assert_eq!(older_page.messages[3].sequence, 5);
        Ok(())
    }

    #[test]
    fn migrates_v4_settings_schema_to_conversations_schema() -> Result<(), Box<dyn Error>> {
        let directory = tempdir()?;
        let path = directory.path().join("lattice.sqlite3");
        let conn = Connection::open(&path)?;
        conn.execute_batch(
            "CREATE TABLE app_settings (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                appearance TEXT NOT NULL CHECK (appearance IN ('system', 'light', 'dark')),
                idle_unload_minutes INTEGER NOT NULL CHECK (
                    idle_unload_minutes >= 1 AND idle_unload_minutes <= 120
                )
            );
            INSERT INTO app_settings (id, revision, appearance, idle_unload_minutes)
            VALUES (1, 1, 'system', 5);
            CREATE TABLE model_load_state (
                id INTEGER PRIMARY KEY CHECK (id = 1),
                revision INTEGER NOT NULL CHECK (revision >= 1),
                ownership_state TEXT NOT NULL,
                owned_identifier TEXT,
                owned_model_key TEXT,
                owned_since_unix_seconds INTEGER
            );
            INSERT INTO model_load_state (id, revision, ownership_state) VALUES (1, 1, 'unknown');
            PRAGMA user_version = 4;",
        )?;
        drop(conn);

        let mut store = ConversationStore::open(&path)?;
        let id = store.begin_or_continue(None, &[user_message("post-migration")])?;
        let detail = store.get(GetConversationRequest {
            conversation_id: id,
            limit: None,
            before_sequence: None,
        })?;
        assert_eq!(detail.messages.len(), 1);
        assert!(super::super::test_support::backup_count(directory.path())? >= 1);
        Ok(())
    }
}
