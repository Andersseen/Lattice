use super::completion::{ChatFinishReason, ChatMessage, ChatRequest, ChatRole};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{
    io::{BufRead, BufReader},
    sync::mpsc::Sender,
    time::Duration,
};

const CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";

/// One event produced by translating the local OpenAI-compatible adapter's
/// wire protocol. Never contains a vendor-shaped field itself; `Delta` and
/// `Completed` are already canonical. Exactly one terminal variant
/// (`Completed`/`Failed`) is ever sent for a given call to
/// [`stream_completion`].
#[derive(Debug)]
pub enum AdapterEvent {
    Delta(String),
    Completed(ChatFinishReason),
    Failed(AppError),
}

#[derive(Serialize)]
struct WireMessage<'a> {
    role: &'a str,
    content: &'a str,
}

#[derive(Serialize)]
struct WireRequest<'a> {
    model: &'a str,
    messages: Vec<WireMessage<'a>>,
    stream: bool,
    max_tokens: u32,
}

#[derive(Deserialize, Default)]
struct WireDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Deserialize, Default)]
struct WireChoice {
    #[serde(default)]
    delta: WireDelta,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Deserialize, Default)]
struct WireChunk {
    #[serde(default)]
    choices: Vec<WireChoice>,
}

fn role_str(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

/// `"stop"` and every other documented-but-unhandled finish reason (for
/// example `"content_filter"`) map to `Stop`, since 0.8 draws no further
/// distinction than "did the model choose to stop" vs. "did it hit the
/// output cap." Only `"length"` gets its own canonical variant.
fn finish_reason_from_str(value: &str) -> ChatFinishReason {
    match value {
        "length" => ChatFinishReason::MaxOutputTokens,
        _ => ChatFinishReason::Stop,
    }
}

/// Blocking call: builds the LM Studio chat-completions request, opens the
/// HTTP connection, and pushes one [`AdapterEvent`] per parsed SSE line
/// (plus exactly one terminal event) into `sender`. Intended to run on its
/// own reader thread; see `completion::run_chat_stream` for the
/// orchestrator that makes this cancellable despite `ureq` having no
/// idle/per-read timeout, only the total `deadline` applied here.
pub fn stream_completion(
    endpoint: &str,
    request: &ChatRequest,
    max_output_tokens: u32,
    deadline: Duration,
    sender: &Sender<AdapterEvent>,
) {
    let wire_messages: Vec<WireMessage> = request
        .messages
        .iter()
        .map(|message: &ChatMessage| WireMessage {
            role: role_str(message.role),
            content: &message.text,
        })
        .collect();
    let wire_request = WireRequest {
        model: &request.model_key,
        messages: wire_messages,
        stream: true,
        max_tokens: max_output_tokens,
    };

    let body = match serde_json::to_vec(&wire_request) {
        Ok(bytes) => bytes,
        Err(_) => {
            let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
                "Could not prepare the chat request.",
            )));
            return;
        }
    };

    let config = ureq::Agent::config_builder()
        .timeout_recv_body(Some(deadline))
        .build();
    let agent: ureq::Agent = config.into();
    let url = format!("{endpoint}{CHAT_COMPLETIONS_PATH}");

    let response = agent
        .post(url.as_str())
        .header("Content-Type", "application/json")
        .send(body);

    let response = match response {
        Ok(response) => response,
        Err(_) => {
            let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
                "Could not reach the local model runtime.",
            )));
            return;
        }
    };

    if !response.status().is_success() {
        let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
            "The local model runtime rejected the request.",
        )));
        return;
    }

    let reader = BufReader::new(response.into_body().into_reader());
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
                    "The local model runtime disconnected before finishing.",
                )));
                return;
            }
        };

        let Some(data) = line.strip_prefix("data: ") else {
            continue;
        };

        if data == "[DONE]" {
            let _ = sender.send(AdapterEvent::Completed(ChatFinishReason::Stop));
            return;
        }

        let chunk: WireChunk = match serde_json::from_str(data) {
            Ok(chunk) => chunk,
            Err(_) => {
                let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
                    "The local model runtime sent an unreadable response.",
                )));
                return;
            }
        };

        let Some(choice) = chunk.choices.into_iter().next() else {
            continue;
        };

        if let Some(text) = choice.delta.content {
            if !text.is_empty() {
                let _ = sender.send(AdapterEvent::Delta(text));
            }
        }

        if let Some(reason) = choice.finish_reason {
            let _ = sender.send(AdapterEvent::Completed(finish_reason_from_str(&reason)));
            return;
        }
    }

    let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
        "The local model runtime disconnected before finishing.",
    )));
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        error::Error,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::mpsc,
        thread,
        time::Duration as StdDuration,
    };

    /// Binds a loopback listener on an OS-assigned port, spawns one thread
    /// that accepts exactly one connection, reads the request up to the end
    /// of headers (the body is left unread; the fixture never needs it),
    /// and hands the stream to `respond` to write a raw HTTP/1.1 response.
    /// This is a disposable test double for `/v1/chat/completions`, not a
    /// second production transport.
    fn spawn_fixture_server(
        respond: impl FnOnce(&mut TcpStream) + Send + 'static,
    ) -> Result<String, Box<dyn Error>> {
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

            respond(&mut stream);
        });

        Ok(format!("http://127.0.0.1:{port}"))
    }

    fn sample_request() -> ChatRequest {
        ChatRequest {
            model_key: "qwen-small".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                text: "hi".to_string(),
            }],
        }
    }

    fn collect(receiver: &mpsc::Receiver<AdapterEvent>) -> Vec<AdapterEvent> {
        let mut events = Vec::new();
        while let Ok(event) = receiver.recv_timeout(StdDuration::from_secs(5)) {
            let is_terminal = matches!(event, AdapterEvent::Completed(_) | AdapterEvent::Failed(_));
            events.push(event);
            if is_terminal {
                break;
            }
        }
        events
    }

    #[test]
    fn streams_deltas_and_completes_on_finish_reason() -> Result<(), Box<dyn Error>> {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}\n\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}\n\n\
                  data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n",
            );
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Delta(first), AdapterEvent::Delta(second), AdapterEvent::Completed(reason)] =
            events.as_slice()
        else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert_eq!(first, "Hello");
        assert_eq!(second, " world");
        assert!(matches!(reason, ChatFinishReason::Stop));
        Ok(())
    }

    /// Confirmed empirically (2026-09-12): `ureq` 3.4.1's `CloseDelimited`
    /// body reader can lose the final chunk of a multi-write response if the
    /// connection closes with zero delay after the last `write_all` — a
    /// race between "more data available" and "remote closed" in its
    /// buffered-input handling, not a bug in this adapter's own parsing
    /// (confirmed by first reproducing the loss, then fixing it only by
    /// adding a trailing delay before the fixture drops its stream, with no
    /// change to `stream_completion` itself). A production llmster
    /// response, and even OS-level scheduling on a real server process, is
    /// very unlikely to close with true zero delay after its last flush,
    /// but this is recorded here rather than silently relied upon — see
    /// tasks.md's real-SSE-sample task, which should specifically watch for
    /// a dropped final chunk against a real installation.
    #[test]
    fn fragmented_sse_lines_split_across_writes_still_parse() -> Result<(), Box<dyn Error>> {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            let _ = stream.write_all(b"data: {\"choices\":[{\"delta\":{\"con");
            thread::sleep(StdDuration::from_millis(20));
            let _ = stream.write_all(b"tent\":\"Hello\"}}]}\n\n");
            thread::sleep(StdDuration::from_millis(20));
            let _ = stream
                .write_all(b"data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n");
            // Trailing delay before the stream is dropped (closing the
            // connection) — see the doc comment above.
            thread::sleep(StdDuration::from_millis(20));
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Delta(text), AdapterEvent::Completed(_)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {:?}", events).into());
        };
        assert_eq!(text, "Hello");
        Ok(())
    }

    #[test]
    fn done_sentinel_without_finish_reason_completes_as_stop() -> Result<(), Box<dyn Error>> {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n\
                  data: [DONE]\n\n",
            );
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Delta(_), AdapterEvent::Completed(reason)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert!(matches!(reason, ChatFinishReason::Stop));
        Ok(())
    }

    #[test]
    fn malformed_delta_line_fails_the_stream_exactly_once() -> Result<(), Box<dyn Error>> {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  data: not-json\n\n",
            );
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Failed(error)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert_eq!(error.code, "chat.failed");
        Ok(())
    }

    #[test]
    fn mid_stream_disconnect_without_terminal_content_fails_once() -> Result<(), Box<dyn Error>> {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}\n\n",
            );
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Delta(_), AdapterEvent::Failed(error)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert_eq!(error.code, "chat.failed");
        Ok(())
    }

    #[test]
    fn non_success_status_fails_the_stream() -> Result<(), Box<dyn Error>> {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n");
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Failed(error)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert_eq!(error.code, "chat.failed");
        Ok(())
    }

    #[test]
    fn a_duplicate_terminal_shaped_payload_is_never_forwarded_twice() -> Result<(), Box<dyn Error>>
    {
        let endpoint = spawn_fixture_server(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}\n\n\
                  data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}\n\n\
                  data: [DONE]\n\n",
            );
        })?;

        let (sender, receiver) = mpsc::channel();
        stream_completion(
            &endpoint,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &sender,
        );
        let events = collect(&receiver);

        let [AdapterEvent::Completed(reason)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert!(matches!(reason, ChatFinishReason::Stop));
        Ok(())
    }
}
