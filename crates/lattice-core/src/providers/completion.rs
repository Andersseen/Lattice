use super::openai_compatible::{
    stream_completion, AdapterEvent, BearerToken, Destination, OpenAiCompatibleTarget,
};
use super::profiles::REMOTE_PROVIDER_KEY;
use crate::conversations::LOCAL_PROVIDER_KEY;
use crate::error::AppError;
use crate::model_runtime::{ModelLoadOwnership, ModelSlotStatus};
use serde::{Deserialize, Serialize};
use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc, Arc,
    },
    thread,
    time::Duration,
};

pub const START_CHAT_STREAM_COMMAND: &str = "start_chat_stream";
pub const CANCEL_CHAT_STREAM_COMMAND: &str = "cancel_chat_stream";

/// Conservative default: no qualified candidate profile's real context
/// window has been read from a live catalog yet (0.7's own catalog-read
/// task is still pending), so this is sized well under both candidate
/// profiles' expected context windows rather than a measured figure.
pub const MAX_OUTPUT_TOKENS: u32 = 1024;

/// Byte-based proxy for a token/context budget, for the same reason
/// `MAX_OUTPUT_TOKENS` is provisional: no verified per-model figure exists
/// yet to size this against.
pub const MAX_PROMPT_CHARS: usize = 32_000;

/// Applied as `ureq`'s `timeout_recv_body`, confirmed to be a total (not
/// idle/per-read) budget for the whole streaming-body phase. Matches the
/// Definition of v1's existing 5-minute run-deadline default.
pub const STREAM_DEADLINE: Duration = Duration::from_secs(300);

/// How often the orchestrator re-checks the shared cancellation flag
/// against its reader-thread channel. Bounds cancellation latency
/// independently of `STREAM_DEADLINE`; not a network timeout.
pub const ORCHESTRATOR_POLL_INTERVAL: Duration = Duration::from_millis(200);

/// Assigns a new run identity. `uuid` is already resolved transitively
/// through the Tauri stack (confirmed in `Cargo.lock`); this only
/// formalizes reliance on it rather than hand-rolling a timestamp/counter
/// scheme, and keeps the dependency confined to `lattice-core` — the
/// Tauri shell never needs `uuid` directly.
pub fn new_chat_run_id() -> String {
    uuid::Uuid::new_v4().to_string()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatRole {
    System,
    User,
    Assistant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatMessage {
    pub role: ChatRole,
    pub text: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRequest {
    pub model_key: String,
    pub messages: Vec<ChatMessage>,
}

/// Which destination one chat request is sent to (0.11). Applies to that
/// request only; nothing about a selection is persisted or dispatched on
/// its own.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatTarget {
    Local,
    Remote { profile_id: String },
}

/// Amended by 0.9 (`conversation-persistence`): the Tauri command's actual
/// request wraps the unchanged `ChatRequest` above with conversation
/// identity. Amended again by 0.11 (`remote-openai-compatible-chat`) with
/// the per-request `target`. `ChatRequest`/`ChatStreamEvent` themselves are
/// untouched by both — see those changes' design.md files for why this
/// wrapper, not the port, carries command-level context.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartChatStreamRequest {
    pub conversation_id: Option<String>,
    pub chat: ChatRequest,
    pub target: ChatTarget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ChatRunHandle {
    pub run_id: String,
    /// Added by 0.9: the conversation this run belongs to — the caller's
    /// own ID if it continued one, or the ID Lattice just created.
    pub conversation_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CancelChatStreamRequest {
    pub run_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum ChatFinishReason {
    Stop,
    MaxOutputTokens,
}

// `rename_all` alone only renames the `kind` tag values (Started -> "started"
// etc.); it does NOT rename each struct variant's own fields — that needs
// the separate `rename_all_fields`. Confirmed by `chat_stream_event_serializes_with_camel_case_kind_tag`
// below, which failed with snake_case field names (`run_id`) until this was added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum ChatStreamEvent {
    Started {
        run_id: String,
        model_key: String,
    },
    Delta {
        run_id: String,
        sequence: u64,
        text: String,
    },
    Completed {
        run_id: String,
        sequence: u64,
        finish_reason: ChatFinishReason,
    },
    Cancelled {
        run_id: String,
        sequence: u64,
    },
    Failed {
        run_id: String,
        sequence: u64,
        error: AppError,
    },
}

/// A resolved, authorized destination for one run: the local runtime
/// endpoint (`CompletionTarget::local`) or a remote profile prepared by
/// `profiles::prepare_remote_target`. Opaque to the desktop shell, which
/// only needs the provenance key and whether the run holds the local model
/// lease; any bearer token inside stays redacted in `Debug`.
#[derive(Debug)]
pub struct CompletionTarget(OpenAiCompatibleTarget);

impl CompletionTarget {
    pub fn local(endpoint: &str) -> Self {
        Self(OpenAiCompatibleTarget::local(endpoint))
    }

    pub(crate) fn remote(endpoint: &str, bearer: Option<BearerToken>) -> Self {
        Self(OpenAiCompatibleTarget::remote(endpoint, bearer))
    }

    /// The provider key recorded on the assistant message this run produces.
    pub fn provider_key(&self) -> &'static str {
        match self.0.destination() {
            Destination::Local => LOCAL_PROVIDER_KEY,
            Destination::Remote => REMOTE_PROVIDER_KEY,
        }
    }

    /// Only local runs lease the managed model; a remote run never blocks
    /// unloading it.
    pub fn holds_local_model_lease(&self) -> bool {
        self.0.destination() == Destination::Local
    }
}

/// Shared by the local and remote authorization paths: refuses a prompt
/// over `MAX_PROMPT_CHARS` before any network call.
pub(crate) fn check_prompt_bound(request: &ChatRequest) -> Result<(), AppError> {
    let prompt_chars: usize = request
        .messages
        .iter()
        .map(|message| message.text.chars().count())
        .sum();
    if prompt_chars > MAX_PROMPT_CHARS {
        return Err(AppError::chat_invalid(
            "The conversation is too long for this model.",
        ));
    }
    Ok(())
}

/// Refuses a local chat request before any network call when the prompt is
/// oversized or the requested model is not the one currently owned and
/// loaded in the managed slot. Never triggers an implicit load.
pub fn authorize_chat_request(
    status: &ModelSlotStatus,
    request: &ChatRequest,
) -> Result<(), AppError> {
    check_prompt_bound(request)?;

    match &status.ownership {
        ModelLoadOwnership::Owned { model_key, .. } if model_key == &request.model_key => Ok(()),
        _ => Err(AppError::chat_invalid(
            "Load the requested model before starting a chat.",
        )),
    }
}

/// Runs one chat stream to completion, cancellation, or failure, calling
/// `on_event` for every canonical event including exactly one terminal
/// event. Blocking; intended to run on a dedicated thread the caller owns
/// (see the desktop shell's `start_chat_stream` command), not the caller's
/// own thread.
///
/// Internally spawns a second, unjoined "reader" thread that performs the
/// actual blocking HTTP call (`openai_compatible::stream_completion`), because
/// `ureq`'s only body timeout is a total budget, not an idle/per-read one:
/// polling here with `ORCHESTRATOR_POLL_INTERVAL` is what makes
/// cancellation responsive even while the reader thread is blocked waiting
/// for the model's first token. A cancelled reader thread is abandoned,
/// not forcibly aborted; it is bounded by `STREAM_DEADLINE` at worst and
/// its output is never forwarded once this function has returned. Since
/// 0.11 the reader also observes `cancel` itself, so it sends no request
/// when cancelled before dispatch and closes the connection at the next
/// received chunk otherwise (best-effort remote cancellation).
pub fn run_chat_stream(
    target: CompletionTarget,
    run_id: String,
    request: ChatRequest,
    cancel: &Arc<AtomicBool>,
    mut on_event: impl FnMut(ChatStreamEvent),
) {
    on_event(ChatStreamEvent::Started {
        run_id: run_id.clone(),
        model_key: request.model_key.clone(),
    });

    let (sender, receiver) = mpsc::channel::<AdapterEvent>();
    let reader_cancel = cancel.clone();
    thread::spawn(move || {
        stream_completion(
            &target.0,
            &request,
            MAX_OUTPUT_TOKENS,
            STREAM_DEADLINE,
            &reader_cancel,
            &sender,
        );
    });

    let mut sequence: u64 = 0;
    loop {
        if cancel.load(Ordering::SeqCst) {
            on_event(ChatStreamEvent::Cancelled { run_id, sequence });
            return;
        }

        let received = receiver.recv_timeout(ORCHESTRATOR_POLL_INTERVAL);
        // A reader that observed cancellation exits without sending, which
        // can surface here as `Disconnected` (or, racing, one last event);
        // cancellation observed at any point wins over what was received.
        if !matches!(received, Err(mpsc::RecvTimeoutError::Timeout))
            && cancel.load(Ordering::SeqCst)
        {
            on_event(ChatStreamEvent::Cancelled { run_id, sequence });
            return;
        }

        match received {
            Ok(AdapterEvent::Delta(text)) => {
                on_event(ChatStreamEvent::Delta {
                    run_id: run_id.clone(),
                    sequence,
                    text,
                });
                sequence += 1;
            }
            Ok(AdapterEvent::Completed(finish_reason)) => {
                on_event(ChatStreamEvent::Completed {
                    run_id,
                    sequence,
                    finish_reason,
                });
                return;
            }
            Ok(AdapterEvent::Failed(error)) => {
                on_event(ChatStreamEvent::Failed {
                    run_id,
                    sequence,
                    error,
                });
                return;
            }
            Err(mpsc::RecvTimeoutError::Timeout) => continue,
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                on_event(ChatStreamEvent::Failed {
                    run_id,
                    sequence,
                    error: AppError::chat_failed("The chat response stopped unexpectedly."),
                });
                return;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model_runtime::ModelDescriptor;
    use std::{
        error::Error,
        io::{Read, Write},
        net::TcpListener,
    };

    fn owned_status(model_key: &str) -> ModelSlotStatus {
        ModelSlotStatus {
            revision: 1,
            installed: vec![ModelDescriptor {
                model_key: model_key.to_string(),
                display_name: model_key.to_string(),
                architecture: None,
                is_llm: true,
                size_bytes: None,
            }],
            loaded: None,
            ownership: ModelLoadOwnership::Owned {
                identifier: "lattice-managed".to_string(),
                model_key: model_key.to_string(),
                loaded_since_unix_seconds: 0,
            },
            last_operation: None,
            last_checked_unix_seconds: None,
            message: String::new(),
        }
    }

    fn request(model_key: &str, text: &str) -> ChatRequest {
        ChatRequest {
            model_key: model_key.to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                text: text.to_string(),
            }],
        }
    }

    /// Locks down the exact wire shape `wire.rs`'s hand-written TypeScript
    /// bindings must match: a `kind`-tagged, `camelCase` discriminated
    /// union, verified here rather than only asserted in design.md prose.
    #[test]
    fn chat_stream_event_serializes_with_camel_case_kind_tag() -> Result<(), Box<dyn Error>> {
        let delta = ChatStreamEvent::Delta {
            run_id: "run-1".to_string(),
            sequence: 2,
            text: "hi".to_string(),
        };
        assert_eq!(
            serde_json::to_value(delta)?,
            serde_json::json!({
                "kind": "delta",
                "runId": "run-1",
                "sequence": 2,
                "text": "hi"
            })
        );

        let completed = ChatStreamEvent::Completed {
            run_id: "run-1".to_string(),
            sequence: 5,
            finish_reason: ChatFinishReason::MaxOutputTokens,
        };
        assert_eq!(
            serde_json::to_value(completed)?,
            serde_json::json!({
                "kind": "completed",
                "runId": "run-1",
                "sequence": 5,
                "finishReason": "maxOutputTokens"
            })
        );

        assert_eq!(
            serde_json::to_value(ChatRole::User)?,
            serde_json::json!("user")
        );
        Ok(())
    }

    #[test]
    fn authorizes_request_naming_the_owned_loaded_model() {
        let status = owned_status("qwen-small");
        assert!(authorize_chat_request(&status, &request("qwen-small", "hi")).is_ok());
    }

    #[test]
    fn refuses_request_naming_a_different_model() -> Result<(), Box<dyn Error>> {
        let status = owned_status("qwen-small");
        let Err(error) = authorize_chat_request(&status, &request("gemma-small", "hi")) else {
            return Err("mismatched model must be refused".into());
        };
        assert_eq!(error.code, "chat.invalid");
        Ok(())
    }

    #[test]
    fn refuses_request_when_slot_is_not_owned() -> Result<(), Box<dyn Error>> {
        let mut status = owned_status("qwen-small");
        status.ownership = ModelLoadOwnership::Unknown;
        let Err(error) = authorize_chat_request(&status, &request("qwen-small", "hi")) else {
            return Err("unowned slot must be refused".into());
        };
        assert_eq!(error.code, "chat.invalid");
        Ok(())
    }

    #[test]
    fn refuses_oversized_prompt_before_any_dispatch() -> Result<(), Box<dyn Error>> {
        let status = owned_status("qwen-small");
        let oversized = "a".repeat(MAX_PROMPT_CHARS + 1);
        let Err(error) = authorize_chat_request(&status, &request("qwen-small", &oversized)) else {
            return Err("oversized prompt must be refused".into());
        };
        assert_eq!(error.code, "chat.invalid");
        Ok(())
    }

    #[test]
    fn cancel_before_any_delta_is_the_only_terminal_event() {
        let cancel = Arc::new(AtomicBool::new(true));
        let mut events = Vec::new();
        run_chat_stream(
            CompletionTarget::local("http://127.0.0.1:0"),
            "run-1".to_string(),
            request("qwen-small", "hi"),
            &cancel,
            |event| events.push(event),
        );

        assert!(matches!(events[0], ChatStreamEvent::Started { .. }));
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[1],
            ChatStreamEvent::Cancelled { sequence: 0, .. }
        ));
    }

    #[test]
    fn unreachable_endpoint_fails_the_run_exactly_once() {
        let cancel = Arc::new(AtomicBool::new(false));
        let mut events = Vec::new();
        run_chat_stream(
            CompletionTarget::local("http://127.0.0.1:1"),
            "run-2".to_string(),
            request("qwen-small", "hi"),
            &cancel,
            |event| events.push(event),
        );

        assert!(matches!(events[0], ChatStreamEvent::Started { .. }));
        assert_eq!(events.len(), 2);
        assert!(matches!(events[1], ChatStreamEvent::Failed { .. }));
    }

    /// A fixture that sends one delta then holds the connection open well
    /// past this test's expected cancellation latency, proving cancel
    /// responds within `ORCHESTRATOR_POLL_INTERVAL` rather than waiting for
    /// `STREAM_DEADLINE` or the reader thread to ever return — the reader
    /// thread here is left blocked and abandoned, exactly as design.md's
    /// "Stream lifecycle, limits and cancellation" describes.
    #[test]
    fn cancel_mid_stream_after_first_delta_is_the_only_terminal_event() -> Result<(), Box<dyn Error>>
    {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();

        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };

            let mut buffer = [0_u8; 1];
            let mut seen = Vec::new();
            while !seen.ends_with(b"\r\n\r\n") {
                if stream.read(&mut buffer).unwrap_or(0) == 0 {
                    return;
                }
                seen.push(buffer[0]);
            }

            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n",
            );
            thread::sleep(Duration::from_secs(2));
        });

        let cancel = Arc::new(AtomicBool::new(false));
        let cancel_setter = cancel.clone();
        let mut events = Vec::new();
        run_chat_stream(
            CompletionTarget::local(&format!("http://127.0.0.1:{port}")),
            "run-3".to_string(),
            request("qwen-small", "hi"),
            &cancel,
            |event| {
                if matches!(event, ChatStreamEvent::Delta { .. }) {
                    cancel_setter.store(true, Ordering::SeqCst);
                }
                events.push(event);
            },
        );

        let [ChatStreamEvent::Started { .. }, ChatStreamEvent::Delta { .. }, ChatStreamEvent::Cancelled { .. }] =
            events.as_slice()
        else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        Ok(())
    }

    #[test]
    fn chat_target_deserializes_from_a_kind_tagged_camel_case_union() -> Result<(), Box<dyn Error>>
    {
        let request: StartChatStreamRequest = serde_json::from_value(serde_json::json!({
            "conversationId": null,
            "chat": { "modelKey": "gpt-test", "messages": [] },
            "target": { "kind": "remote", "profileId": "profile-1" }
        }))?;
        assert_eq!(
            request.target,
            ChatTarget::Remote {
                profile_id: "profile-1".to_string()
            }
        );

        let local: ChatTarget = serde_json::from_value(serde_json::json!({ "kind": "local" }))?;
        assert_eq!(local, ChatTarget::Local);

        let missing_target = serde_json::from_value::<StartChatStreamRequest>(serde_json::json!({
            "conversationId": null,
            "chat": { "modelKey": "m", "messages": [] }
        }));
        assert!(
            missing_target.is_err(),
            "the target is required, never implied"
        );
        Ok(())
    }

    #[test]
    fn completion_targets_report_provenance_and_lease_scope() {
        let local = CompletionTarget::local("http://127.0.0.1:1234");
        assert_eq!(local.provider_key(), LOCAL_PROVIDER_KEY);
        assert!(local.holds_local_model_lease());

        let remote = CompletionTarget::remote("https://api.example.com/v1", None);
        assert_eq!(remote.provider_key(), REMOTE_PROVIDER_KEY);
        assert!(!remote.holds_local_model_lease());
    }

    /// Two protocol fixtures — the local destination and the remote one —
    /// receive the same canonical history and produce the same canonical
    /// events, differing only in run identity: the substitution 0.8's port
    /// was designed for, proven rather than asserted.
    #[test]
    fn the_same_canonical_history_streams_identically_through_both_destinations(
    ) -> Result<(), Box<dyn Error>> {
        use super::super::openai_compatible::test_support::{spawn_capturing_fixture, write_sse};

        let stream = |stream: &mut std::net::TcpStream| {
            write_sse(
                stream,
                &[
                    "data: {\"choices\":[{\"delta\":{\"content\":\"Same\"}}]}",
                    "data: {\"choices\":[{\"delta\":{\"content\":\" answer\"},\"finish_reason\":\"length\"}]}",
                ],
            );
        };
        let (local_endpoint, local_captured) = spawn_capturing_fixture(stream)?;
        let (remote_endpoint, remote_captured) = spawn_capturing_fixture(stream)?;

        let history = ChatRequest {
            model_key: "shared-model".to_string(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::User,
                    text: "local question".to_string(),
                },
                ChatMessage {
                    role: ChatRole::Assistant,
                    text: "local answer".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    text: "now remote".to_string(),
                },
            ],
        };

        let mut outcomes = Vec::new();
        for (target, run_id) in [
            (CompletionTarget::local(&local_endpoint), "local-run"),
            (
                CompletionTarget::remote(&format!("{remote_endpoint}/v1"), None),
                "remote-run",
            ),
        ] {
            let mut events = Vec::new();
            run_chat_stream(
                target,
                run_id.to_string(),
                history.clone(),
                &Arc::new(AtomicBool::new(false)),
                |event| events.push(serde_json::to_value(event).unwrap_or_default()),
            );
            for event in &mut events {
                if let Some(object) = event.as_object_mut() {
                    object.remove("runId");
                }
            }
            outcomes.push(events);
        }

        assert_eq!(outcomes[0], outcomes[1]);
        assert_eq!(outcomes[0].len(), 4);
        assert_eq!(outcomes[0][3]["finishReason"], "maxOutputTokens");

        let local_body = local_captured
            .recv_timeout(Duration::from_secs(5))?
            .json_body()?;
        let remote_body = remote_captured
            .recv_timeout(Duration::from_secs(5))?
            .json_body()?;
        assert_eq!(local_body["messages"], remote_body["messages"]);
        assert_eq!(local_body["model"], remote_body["model"]);
        Ok(())
    }
}
