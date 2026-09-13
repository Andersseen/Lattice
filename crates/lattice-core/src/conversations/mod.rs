//! Conversation/message identity, ordering, generation status, and the
//! bounded checkpoint policy for a streaming assistant reply. Persistence
//! against these types lives in `storage::conversations`, exactly as
//! `model_runtime`'s domain types are separate from `storage::runtime`'s
//! repository. Reuses `providers::completion::ChatRole`/`ChatMessage`
//! rather than duplicating a role/message shape.

use crate::providers::ChatRole;
use serde::{Deserialize, Serialize};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const LIST_CONVERSATIONS_COMMAND: &str = "list_conversations";
pub const GET_CONVERSATION_COMMAND: &str = "get_conversation";
pub const DELETE_CONVERSATION_COMMAND: &str = "delete_conversation";

/// How many accumulated `Delta` events trigger a checkpoint write, whichever
/// of this or `CHECKPOINT_MIN_INTERVAL` is reached first. Bounds storage
/// write frequency without waiting for the full reply; see design.md's
/// "Checkpointing and message persistence".
pub const CHECKPOINT_DELTA_BATCH: u32 = 20;

/// Minimum elapsed time since the last checkpoint before another one is
/// written, independent of delta count. The terminal event always flushes
/// synchronously regardless of this interval.
pub const CHECKPOINT_MIN_INTERVAL: Duration = Duration::from_millis(1000);

/// Conversation titles are derived once from the first user message and
/// truncated to this many characters; never recomputed (no rename UI in
/// this change).
pub const MAX_TITLE_CHARS: usize = 80;

/// Server-enforced upper bound on `ListConversationsRequest.limit`,
/// regardless of what the caller requests.
pub const MAX_CONVERSATIONS_PAGE_SIZE: u32 = 50;

/// Server-enforced upper bound on `GetConversationRequest.limit`.
pub const MAX_MESSAGES_PAGE_SIZE: u32 = 100;

const DEFAULT_CONVERSATIONS_PAGE_SIZE: u32 = 20;
const DEFAULT_MESSAGES_PAGE_SIZE: u32 = 50;

/// The only provider key 0.9 ever records: `providers::local_openai`'s
/// fixed local adapter. Recorded per assistant message (not per
/// conversation) from the start, so 0.11+ remote providers do not need a
/// schema migration to relocate this column once a conversation's messages
/// can span more than one provider.
pub const LOCAL_PROVIDER_KEY: &str = "local-openai-compatible";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GenerationStatus {
    Complete,
    Streaming,
    Cancelled,
    Failed,
    Interrupted,
}

impl GenerationStatus {
    pub fn as_storage_value(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Streaming => "streaming",
            Self::Cancelled => "cancelled",
            Self::Failed => "failed",
            Self::Interrupted => "interrupted",
        }
    }

    pub fn from_storage_value(value: &str) -> Option<Self> {
        match value {
            "complete" => Some(Self::Complete),
            "streaming" => Some(Self::Streaming),
            "cancelled" => Some(Self::Cancelled),
            "failed" => Some(Self::Failed),
            "interrupted" => Some(Self::Interrupted),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Message {
    pub id: String,
    pub conversation_id: String,
    pub sequence: u64,
    pub role: ChatRole,
    pub text: String,
    pub status: GenerationStatus,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model_key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error_message: Option<String>,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Conversation {
    pub id: String,
    pub title: String,
    pub created_at_unix_seconds: u64,
    pub updated_at_unix_seconds: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub updated_at_unix_seconds: u64,
    pub message_count: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub last_status: Option<GenerationStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationCursor {
    pub updated_at_unix_seconds: u64,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsRequest {
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub before: Option<ConversationCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListConversationsResponse {
    pub conversations: Vec<ConversationSummary>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub next_before: Option<ConversationCursor>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetConversationRequest {
    pub conversation_id: String,
    #[serde(default)]
    pub limit: Option<u32>,
    #[serde(default)]
    pub before_sequence: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationDetail {
    pub conversation: Conversation,
    pub messages: Vec<Message>,
    pub has_more_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteConversationRequest {
    pub conversation_id: String,
}

/// Clamps a caller-requested page size to a positive value not exceeding
/// `max`, defaulting to `default` when the caller did not specify one.
/// Bounds pages server-side regardless of what the caller asks for.
pub(crate) fn bounded_page_size(requested: Option<u32>, default: u32, max: u32) -> u32 {
    requested
        .filter(|value| *value > 0)
        .unwrap_or(default)
        .min(max)
}

pub(crate) fn conversations_page_size(requested: Option<u32>) -> u32 {
    bounded_page_size(
        requested,
        DEFAULT_CONVERSATIONS_PAGE_SIZE,
        MAX_CONVERSATIONS_PAGE_SIZE,
    )
}

pub(crate) fn messages_page_size(requested: Option<u32>) -> u32 {
    bounded_page_size(
        requested,
        DEFAULT_MESSAGES_PAGE_SIZE,
        MAX_MESSAGES_PAGE_SIZE,
    )
}

/// Assigns a new conversation identity. Application-generated, never a
/// provider/vendor identifier — see the conversations spec's "Lattice SHALL
/// Identify Conversations And Messages With Application-Generated Identity".
pub fn new_conversation_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// Assigns a new message identity, same rationale as `new_conversation_id`.
pub fn new_message_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

pub(crate) fn unix_timestamp_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_secs())
}

/// Derives a conversation title once, from the first user message's text:
/// trimmed, collapsed to a single line, and truncated to `MAX_TITLE_CHARS`
/// characters. Falls back to a fixed placeholder for an empty/whitespace
/// message so every conversation has a non-empty title. Never recomputed
/// after creation (no rename UI in this change).
pub fn derive_conversation_title(first_user_message_text: &str) -> String {
    let collapsed = first_user_message_text
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ");
    let trimmed = collapsed.trim();

    if trimmed.is_empty() {
        return "New conversation".to_string();
    }

    let truncated: String = trimmed.chars().take(MAX_TITLE_CHARS).collect();
    if trimmed.chars().count() > MAX_TITLE_CHARS {
        format!("{truncated}\u{2026}")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn derives_title_from_first_user_message() {
        assert_eq!(derive_conversation_title("Hello there"), "Hello there");
    }

    #[test]
    fn collapses_internal_whitespace_in_derived_title() {
        assert_eq!(
            derive_conversation_title("  Hello\n  there\tfriend  "),
            "Hello there friend"
        );
    }

    #[test]
    fn falls_back_to_placeholder_for_blank_message() {
        assert_eq!(derive_conversation_title("   \n\t  "), "New conversation");
    }

    #[test]
    fn truncates_long_titles_with_an_ellipsis() {
        let long_message = "a".repeat(MAX_TITLE_CHARS + 10);
        let title = derive_conversation_title(&long_message);

        assert_eq!(title.chars().count(), MAX_TITLE_CHARS + 1);
        assert!(title.ends_with('\u{2026}'));
    }

    #[test]
    fn generates_distinct_uuid_identities() {
        let first = new_conversation_id();
        let second = new_conversation_id();

        assert_ne!(first, second);
        assert_eq!(first.len(), 36);
        assert_ne!(new_message_id(), new_message_id());
    }

    #[test]
    fn generation_status_round_trips_through_storage_values() {
        for status in [
            GenerationStatus::Complete,
            GenerationStatus::Streaming,
            GenerationStatus::Cancelled,
            GenerationStatus::Failed,
            GenerationStatus::Interrupted,
        ] {
            let stored = status.as_storage_value();
            assert_eq!(GenerationStatus::from_storage_value(stored), Some(status));
        }

        assert_eq!(GenerationStatus::from_storage_value("unknown"), None);
    }

    #[test]
    fn clamps_page_size_to_bounds() {
        assert_eq!(
            conversations_page_size(None),
            DEFAULT_CONVERSATIONS_PAGE_SIZE
        );
        assert_eq!(
            conversations_page_size(Some(0)),
            DEFAULT_CONVERSATIONS_PAGE_SIZE
        );
        assert_eq!(
            conversations_page_size(Some(10_000)),
            MAX_CONVERSATIONS_PAGE_SIZE
        );
        assert_eq!(conversations_page_size(Some(5)), 5);

        assert_eq!(messages_page_size(Some(10_000)), MAX_MESSAGES_PAGE_SIZE);
    }
}
