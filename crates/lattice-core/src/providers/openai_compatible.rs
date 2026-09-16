//! The one adapter that knows the OpenAI-compatible chat-completions wire
//! protocol (0.8 `local_openai`, generalized by 0.11). It serves two
//! destinations that speak the same pinned protocol subset and differ only
//! in URL, authentication, output-limit field, timeouts and how failures
//! are named:
//!
//! - **Local**: llmster's loopback `{endpoint}/v1/chat/completions`, no
//!   authorization, `max_tokens` (LM Studio's documented field), failures
//!   reported as 0.8's `chat.failed`.
//! - **Remote**: a profile's HTTPS `{endpoint}/chat/completions`, an
//!   optional bearer token, `max_completion_tokens` (OpenAI's current field;
//!   `max_tokens` is deprecated there), failures normalized to `provider.*`.
//!
//! Both destinations never follow redirects, never use an
//! environment-configured proxy, never read a non-success response body,
//! bound every stream line, and stop reading once the run is cancelled.
//! See `openspec/changes/remote-openai-compatible-chat/design.md`.

use super::completion::{ChatFinishReason, ChatMessage, ChatRequest, ChatRole};
use crate::error::AppError;
use serde::{Deserialize, Serialize};
use std::{
    fmt,
    io::{self, BufRead, BufReader, Read},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    time::Duration,
};

const LOCAL_CHAT_COMPLETIONS_PATH: &str = "/v1/chat/completions";
const REMOTE_CHAT_COMPLETIONS_PATH: &str = "/chat/completions";

/// Upper bound on one Server-Sent Events line. A longer line fails the run
/// instead of growing a buffer without limit; matches the Definition of
/// v1's 1 MiB protocol-message budget.
pub const MAX_STREAM_LINE_BYTES: usize = 1024 * 1024;

/// Connection-establishment bound for a remote destination (DNS, TCP and
/// TLS handshake). The whole remote call is additionally bounded by the
/// run deadline passed to [`stream_completion`].
pub const REMOTE_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

const MAX_BEARER_TOKEN_BYTES: usize = 4096;

/// One event produced by translating the wire protocol. Never contains a
/// vendor-shaped field itself; `Delta` and `Completed` are already
/// canonical. At most one terminal variant (`Completed`/`Failed`) is ever
/// sent for a given call to [`stream_completion`], and none at all once the
/// run has been cancelled.
#[derive(Debug)]
pub enum AdapterEvent {
    Delta(String),
    Completed(ChatFinishReason),
    Failed(AppError),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Destination {
    Local,
    Remote,
}

/// A remote credential prepared for exactly one request's `Authorization`
/// header. Has no `Serialize` impl and a redacted `Debug`, so it cannot
/// reach IPC, logs or error text by accident; it is dropped when the
/// request ends and is never cached.
pub struct BearerToken(String);

impl BearerToken {
    /// Accepts the resolved secret bytes, trimmed of surrounding ASCII
    /// whitespace, only when they are 1–4096 visible ASCII characters — the
    /// only bytes an HTTP header token can carry. The refusal message never
    /// echoes the value.
    pub fn from_secret(secret: Vec<u8>) -> Result<Self, AppError> {
        let trimmed = secret.trim_ascii();
        if trimmed.is_empty()
            || trimmed.len() > MAX_BEARER_TOKEN_BYTES
            || !trimmed.iter().all(u8::is_ascii_graphic)
        {
            return Err(AppError::provider_invalid(
                "The bound credential cannot be used as an API key.",
            ));
        }
        let token = String::from_utf8(trimmed.to_vec()).map_err(|_| {
            AppError::provider_invalid("The bound credential cannot be used as an API key.")
        })?;
        Ok(Self(token))
    }

    fn header_value(&self) -> Option<ureq::http::HeaderValue> {
        let mut value = ureq::http::HeaderValue::try_from(format!("Bearer {}", self.0)).ok()?;
        value.set_sensitive(true);
        Some(value)
    }
}

impl fmt::Debug for BearerToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("BearerToken(<redacted>)")
    }
}

/// Where and how one chat request is sent. Constructed only through
/// [`OpenAiCompatibleTarget::local`] or [`OpenAiCompatibleTarget::remote`];
/// the remote endpoint has already passed `profiles::validate_endpoint`.
#[derive(Debug)]
pub struct OpenAiCompatibleTarget {
    destination: Destination,
    url: String,
    bearer: Option<BearerToken>,
}

impl OpenAiCompatibleTarget {
    pub fn local(endpoint: &str) -> Self {
        Self {
            destination: Destination::Local,
            url: format!("{endpoint}{LOCAL_CHAT_COMPLETIONS_PATH}"),
            bearer: None,
        }
    }

    pub fn remote(endpoint: &str, bearer: Option<BearerToken>) -> Self {
        Self {
            destination: Destination::Remote,
            url: format!("{endpoint}{REMOTE_CHAT_COMPLETIONS_PATH}"),
            bearer,
        }
    }

    pub fn destination(&self) -> Destination {
        self.destination
    }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    max_tokens: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    max_completion_tokens: Option<u32>,
}

#[derive(Deserialize, Default)]
struct WireDelta {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    refusal: Option<String>,
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
    #[serde(default)]
    error: Option<serde_json::Value>,
}

fn role_str(role: ChatRole) -> &'static str {
    match role {
        ChatRole::System => "system",
        ChatRole::User => "user",
        ChatRole::Assistant => "assistant",
    }
}

/// `"length"` is the only finish reason with its own canonical variant.
/// `"stop"` and every other documented value (`content_filter`, and
/// `tool_calls`/`function_call`, which cannot legitimately occur since no
/// tools are ever sent) map to `Stop`.
fn finish_reason_from_str(value: &str) -> ChatFinishReason {
    match value {
        "length" => ChatFinishReason::MaxOutputTokens,
        _ => ChatFinishReason::Stop,
    }
}

/// An assistant turn with no text (a reply that failed before its first
/// token) is omitted from the wire body only; canonical history is not
/// touched.
fn wire_messages(messages: &[ChatMessage]) -> Vec<WireMessage<'_>> {
    messages
        .iter()
        .filter(|message| !(message.role == ChatRole::Assistant && message.text.trim().is_empty()))
        .map(|message| WireMessage {
            role: role_str(message.role),
            content: &message.text,
        })
        .collect()
}

fn build_body(
    destination: Destination,
    request: &ChatRequest,
    max_output_tokens: u32,
) -> serde_json::Result<Vec<u8>> {
    let (max_tokens, max_completion_tokens) = match destination {
        Destination::Local => (Some(max_output_tokens), None),
        Destination::Remote => (None, Some(max_output_tokens)),
    };
    serde_json::to_vec(&WireRequest {
        model: &request.model_key,
        messages: wire_messages(&request.messages),
        stream: true,
        max_tokens,
        max_completion_tokens,
    })
}

fn build_agent(destination: Destination, deadline: Duration) -> ureq::Agent {
    let builder = ureq::Agent::config_builder()
        .http_status_as_error(false)
        .max_redirects(0)
        .proxy(None);
    let config = match destination {
        Destination::Local => builder.timeout_recv_body(Some(deadline)),
        Destination::Remote => builder
            .timeout_connect(Some(REMOTE_CONNECT_TIMEOUT))
            .timeout_global(Some(deadline)),
    }
    .build();
    config.into()
}

fn send_failure(destination: Destination, error: &ureq::Error) -> AppError {
    match destination {
        Destination::Local => AppError::chat_failed("Could not reach the local model runtime."),
        Destination::Remote => match error {
            ureq::Error::Timeout(_) => {
                AppError::provider_timeout("The provider did not respond in time.")
            }
            ureq::Error::Tls(_) | ureq::Error::Rustls(_) => AppError::provider_unavailable(
                "Could not establish a secure connection to the provider.",
            ),
            _ => AppError::provider_unavailable("Could not reach the provider."),
        },
    }
}

fn status_failure(destination: Destination, status: u16) -> AppError {
    match destination {
        Destination::Local => {
            AppError::chat_failed("The local model runtime rejected the request.")
        }
        Destination::Remote => match status {
            300..=399 => AppError::provider_rejected(
                "The provider redirected the request; Lattice does not follow redirects.",
            ),
            401 | 403 => AppError::provider_auth_failed(
                "The provider rejected the credential or denied access.",
            ),
            408 => AppError::provider_timeout("The provider did not respond in time."),
            429 => AppError::provider_rate_limited(
                "The provider's rate or usage limit was reached. Try again later.",
            ),
            500..=599 => AppError::provider_unavailable("The provider is unavailable right now."),
            _ => AppError::provider_rejected(
                "The provider rejected the request. Check the endpoint and model name.",
            ),
        },
    }
}

fn disconnected(destination: Destination) -> AppError {
    match destination {
        Destination::Local => {
            AppError::chat_failed("The local model runtime disconnected before finishing.")
        }
        Destination::Remote => AppError::chat_failed("The provider disconnected before finishing."),
    }
}

fn unreadable(destination: Destination) -> AppError {
    match destination {
        Destination::Local => {
            AppError::chat_failed("The local model runtime sent an unreadable response.")
        }
        Destination::Remote => AppError::chat_failed("The provider sent an unreadable response."),
    }
}

fn deadline_elapsed(destination: Destination) -> AppError {
    match destination {
        Destination::Local => AppError::chat_failed("Generation took too long and was stopped."),
        Destination::Remote => {
            AppError::provider_timeout("Generation took too long and was stopped.")
        }
    }
}

fn stream_error(destination: Destination) -> AppError {
    match destination {
        Destination::Local => {
            AppError::chat_failed("The local model runtime reported an error during the response.")
        }
        Destination::Remote => {
            AppError::provider_rejected("The provider reported an error during the response.")
        }
    }
}

/// `ureq` surfaces a body-phase deadline as an `io::Error` wrapping its own
/// `Error::Timeout`, not as `ErrorKind::TimedOut`; both are checked.
fn is_timeout(error: &io::Error) -> bool {
    error.kind() == io::ErrorKind::TimedOut
        || error
            .get_ref()
            .and_then(|inner| inner.downcast_ref::<ureq::Error>())
            .is_some_and(|inner| matches!(inner, ureq::Error::Timeout(_)))
}

enum LineRead {
    Line(String),
    Eof,
    Unreadable,
}

/// Reads one `\n`-terminated line (tolerating `\r\n`) without ever holding
/// more than `MAX_STREAM_LINE_BYTES` plus its terminator in `buffer`. An
/// over-long or non-UTF-8 line is reported as `Unreadable`.
fn read_bounded_line(reader: &mut impl BufRead, buffer: &mut Vec<u8>) -> io::Result<LineRead> {
    buffer.clear();
    let limit = MAX_STREAM_LINE_BYTES as u64 + 2;
    let read = reader.by_ref().take(limit).read_until(b'\n', buffer)?;
    if read == 0 {
        return Ok(LineRead::Eof);
    }

    let terminated = buffer.last() == Some(&b'\n');
    if terminated {
        buffer.pop();
        if buffer.last() == Some(&b'\r') {
            buffer.pop();
        }
    } else if read as u64 == limit {
        return Ok(LineRead::Unreadable);
    }

    if buffer.len() > MAX_STREAM_LINE_BYTES {
        return Ok(LineRead::Unreadable);
    }

    Ok(match std::str::from_utf8(buffer) {
        Ok(line) => LineRead::Line(line.to_owned()),
        Err(_) => LineRead::Unreadable,
    })
}

/// The payload of an SSE `data:` field (one optional leading space removed).
/// Comments (`:`), `event:`/`id:`/`retry:` fields and blank lines are not
/// data and return `None`.
fn sse_data(line: &str) -> Option<&str> {
    let value = line.strip_prefix("data:")?;
    Some(value.strip_prefix(' ').unwrap_or(value))
}

/// Blocking call: builds the destination's chat-completions request, sends
/// it, and pushes one [`AdapterEvent`] per parsed stream chunk plus exactly
/// one terminal event into `sender`. Intended to run on its own reader
/// thread; see `completion::run_chat_stream` for the orchestrator that makes
/// cancellation prompt.
///
/// `cancel` makes cancellation *best-effort at the connection level*: a
/// run cancelled before dispatch sends no request at all, and a run
/// cancelled mid-stream stops at the next received line, dropping the
/// response and closing the socket so a remote server stops generating. A
/// reader blocked before the first byte is only released by that byte or
/// by `deadline`. Nothing is forwarded after cancellation is observed.
pub fn stream_completion(
    target: &OpenAiCompatibleTarget,
    request: &ChatRequest,
    max_output_tokens: u32,
    deadline: Duration,
    cancel: &AtomicBool,
    sender: &Sender<AdapterEvent>,
) {
    let destination = target.destination;

    let Ok(body) = build_body(destination, request, max_output_tokens) else {
        let _ = sender.send(AdapterEvent::Failed(AppError::chat_failed(
            "Could not prepare the chat request.",
        )));
        return;
    };

    let mut http_request = build_agent(destination, deadline)
        .post(target.url.as_str())
        .header("Content-Type", "application/json")
        .header("Accept", "text/event-stream");
    if let Some(bearer) = &target.bearer {
        let Some(authorization) = bearer.header_value() else {
            let _ = sender.send(AdapterEvent::Failed(AppError::provider_invalid(
                "The bound credential cannot be used as an API key.",
            )));
            return;
        };
        http_request = http_request.header("Authorization", authorization);
    }

    if cancel.load(Ordering::SeqCst) {
        return;
    }

    let response = match http_request.send(body) {
        Ok(response) => response,
        Err(error) => {
            if !cancel.load(Ordering::SeqCst) {
                let _ = sender.send(AdapterEvent::Failed(send_failure(destination, &error)));
            }
            return;
        }
    };

    if !response.status().is_success() {
        if !cancel.load(Ordering::SeqCst) {
            let _ = sender.send(AdapterEvent::Failed(status_failure(
                destination,
                response.status().as_u16(),
            )));
        }
        return;
    }

    let mut reader = BufReader::new(response.into_body().into_reader());
    let mut buffer = Vec::new();
    loop {
        let read = read_bounded_line(&mut reader, &mut buffer);
        if cancel.load(Ordering::SeqCst) {
            return;
        }

        let line = match read {
            Ok(LineRead::Line(line)) => line,
            Ok(LineRead::Eof) => {
                let _ = sender.send(AdapterEvent::Failed(disconnected(destination)));
                return;
            }
            Ok(LineRead::Unreadable) => {
                let _ = sender.send(AdapterEvent::Failed(unreadable(destination)));
                return;
            }
            Err(error) => {
                let failure = if is_timeout(&error) {
                    deadline_elapsed(destination)
                } else {
                    disconnected(destination)
                };
                let _ = sender.send(AdapterEvent::Failed(failure));
                return;
            }
        };

        let Some(data) = sse_data(&line) else {
            continue;
        };

        if data == "[DONE]" {
            let _ = sender.send(AdapterEvent::Completed(ChatFinishReason::Stop));
            return;
        }

        let chunk: WireChunk = match serde_json::from_str(data) {
            Ok(chunk) => chunk,
            Err(_) => {
                let _ = sender.send(AdapterEvent::Failed(unreadable(destination)));
                return;
            }
        };

        if chunk.error.is_some() {
            let _ = sender.send(AdapterEvent::Failed(stream_error(destination)));
            return;
        }

        let Some(choice) = chunk.choices.into_iter().next() else {
            continue;
        };

        for text in [choice.delta.content, choice.delta.refusal]
            .into_iter()
            .flatten()
        {
            if !text.is_empty() {
                let _ = sender.send(AdapterEvent::Delta(text));
            }
        }

        if let Some(reason) = choice.finish_reason {
            let _ = sender.send(AdapterEvent::Completed(finish_reason_from_str(&reason)));
            return;
        }
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        error::Error,
        io::{Read, Write},
        net::{TcpListener, TcpStream},
        sync::mpsc,
        thread,
        time::Duration,
    };

    /// What a disposable fixture server observed about the one request it
    /// accepted: the request line, lower-cased header lines, and the body.
    #[derive(Debug, Clone, Default)]
    pub(crate) struct CapturedRequest {
        pub(crate) request_line: String,
        pub(crate) headers: Vec<String>,
        pub(crate) body: String,
    }

    impl CapturedRequest {
        pub(crate) fn header(&self, name: &str) -> Option<&str> {
            let prefix = format!("{}:", name.to_ascii_lowercase());
            self.headers.iter().find_map(|line| {
                line.to_ascii_lowercase()
                    .starts_with(&prefix)
                    .then(|| line[prefix.len()..].trim())
            })
        }

        pub(crate) fn json_body(&self) -> Result<serde_json::Value, Box<dyn Error>> {
            Ok(serde_json::from_str(&self.body)?)
        }
    }

    /// Binds a loopback listener on an OS-assigned port and accepts exactly
    /// one connection on a background thread: reads the request head and
    /// its `Content-Length` body, reports them through the returned
    /// receiver, then hands the stream to `respond`. A disposable test
    /// double for an OpenAI-compatible endpoint, not a production transport.
    pub(crate) fn spawn_capturing_fixture(
        respond: impl FnOnce(&mut TcpStream) + Send + 'static,
    ) -> Result<(String, mpsc::Receiver<CapturedRequest>), Box<dyn Error>> {
        let listener = TcpListener::bind("127.0.0.1:0")?;
        let port = listener.local_addr()?.port();
        let (sender, receiver) = mpsc::channel();

        thread::spawn(move || {
            let Ok((mut stream, _)) = listener.accept() else {
                return;
            };
            let Some(captured) = read_request(&mut stream) else {
                return;
            };
            let _ = sender.send(captured);
            respond(&mut stream);
        });

        Ok((format!("http://127.0.0.1:{port}"), receiver))
    }

    fn read_request(stream: &mut TcpStream) -> Option<CapturedRequest> {
        let mut byte = [0_u8; 1];
        let mut head = Vec::new();
        while !head.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).ok()? == 0 {
                return None;
            }
            head.push(byte[0]);
        }

        let head = String::from_utf8(head).ok()?;
        let mut lines = head.split("\r\n").filter(|line| !line.is_empty());
        let request_line = lines.next()?.to_string();
        let headers: Vec<String> = lines.map(str::to_string).collect();
        let content_length = headers
            .iter()
            .find_map(|line| {
                line.to_ascii_lowercase()
                    .strip_prefix("content-length:")
                    .map(|value| value.trim().parse::<usize>().unwrap_or(0))
            })
            .unwrap_or(0);

        let mut body = vec![0_u8; content_length];
        stream.read_exact(&mut body).ok()?;
        Some(CapturedRequest {
            request_line,
            headers,
            body: String::from_utf8(body).ok()?,
        })
    }

    pub(crate) fn write_sse(stream: &mut TcpStream, events: &[&str]) {
        let _ = stream.write_all(
            b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
        );
        for event in events {
            let _ = stream.write_all(format!("{event}\n\n").as_bytes());
        }
        // See `fragmented_sse_lines_split_across_writes_still_parse`: a
        // zero-delay close can race `ureq`'s close-delimited body reader.
        thread::sleep(Duration::from_millis(20));
    }

    /// A listener that must never be contacted; `was_contacted` reports
    /// whether any connection arrived.
    pub(crate) struct UntouchedListener {
        listener: TcpListener,
    }

    impl UntouchedListener {
        pub(crate) fn bind() -> Result<Self, Box<dyn Error>> {
            let listener = TcpListener::bind("127.0.0.1:0")?;
            listener.set_nonblocking(true)?;
            Ok(Self { listener })
        }

        pub(crate) fn port(&self) -> Result<u16, Box<dyn Error>> {
            Ok(self.listener.local_addr()?.port())
        }

        pub(crate) fn was_contacted(&self) -> bool {
            thread::sleep(Duration::from_millis(100));
            self.listener.accept().is_ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::{spawn_capturing_fixture, write_sse, UntouchedListener};
    use super::*;
    use std::{
        error::Error,
        io::{Read, Write},
        sync::{mpsc, Arc},
        thread,
        time::{Duration as StdDuration, Instant},
    };

    const SECRET_BODY_MARKER: &str = "VENDOR-DETAIL-sk-leaked-0123";

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

    fn run(target: &OpenAiCompatibleTarget, request: &ChatRequest) -> Vec<AdapterEvent> {
        run_with_deadline(target, request, Duration::from_secs(5))
    }

    fn run_with_deadline(
        target: &OpenAiCompatibleTarget,
        request: &ChatRequest,
        deadline: Duration,
    ) -> Vec<AdapterEvent> {
        let (sender, receiver) = mpsc::channel();
        stream_completion(
            target,
            request,
            64,
            deadline,
            &AtomicBool::new(false),
            &sender,
        );
        collect(&receiver)
    }

    fn local(endpoint: &str) -> OpenAiCompatibleTarget {
        OpenAiCompatibleTarget::local(endpoint)
    }

    fn remote(endpoint: &str, secret: Option<&[u8]>) -> Result<OpenAiCompatibleTarget, AppError> {
        let bearer = secret
            .map(|secret| BearerToken::from_secret(secret.to_vec()))
            .transpose()?;
        Ok(OpenAiCompatibleTarget::remote(
            &format!("{endpoint}/v1"),
            bearer,
        ))
    }

    fn failed_code(events: &[AdapterEvent]) -> Result<&'static str, Box<dyn Error>> {
        match events {
            [AdapterEvent::Failed(error)] => Ok(error.code),
            other => Err(format!("expected exactly one failure, got {other:?}").into()),
        }
    }

    #[test]
    fn streams_deltas_and_completes_on_finish_reason() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            write_sse(
                stream,
                &[
                    "data: {\"choices\":[{\"delta\":{\"content\":\"Hello\"}}]}",
                    "data: {\"choices\":[{\"delta\":{\"content\":\" world\"}}]}",
                    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}",
                ],
            );
        })?;

        let events = run(&local(&endpoint), &sample_request());

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
    /// the 0.8 tasks.md real-SSE-sample task, which should specifically
    /// watch for a dropped final chunk against a real installation.
    #[test]
    fn fragmented_sse_lines_split_across_writes_still_parse() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
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

        let events = run(&local(&endpoint), &sample_request());

        let [AdapterEvent::Delta(text), AdapterEvent::Completed(_)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {:?}", events).into());
        };
        assert_eq!(text, "Hello");
        Ok(())
    }

    #[test]
    fn done_sentinel_without_finish_reason_completes_as_stop() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            write_sse(
                stream,
                &[
                    "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}",
                    "data: [DONE]",
                ],
            );
        })?;

        let events = run(&local(&endpoint), &sample_request());

        let [AdapterEvent::Delta(_), AdapterEvent::Completed(reason)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert!(matches!(reason, ChatFinishReason::Stop));
        Ok(())
    }

    #[test]
    fn malformed_delta_line_fails_the_stream_exactly_once() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) =
            spawn_capturing_fixture(|stream| write_sse(stream, &["data: not-json"]))?;

        let events = run(&local(&endpoint), &sample_request());

        assert_eq!(failed_code(&events)?, "chat.failed");
        Ok(())
    }

    #[test]
    fn mid_stream_disconnect_without_terminal_content_fails_once() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            write_sse(
                stream,
                &["data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}"],
            );
        })?;

        let events = run(&local(&endpoint), &sample_request());

        let [AdapterEvent::Delta(_), AdapterEvent::Failed(error)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert_eq!(error.code, "chat.failed");
        Ok(())
    }

    #[test]
    fn non_success_status_fails_the_stream() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            let _ = stream
                .write_all(b"HTTP/1.1 500 Internal Server Error\r\nConnection: close\r\n\r\n");
        })?;

        let events = run(&local(&endpoint), &sample_request());

        assert_eq!(failed_code(&events)?, "chat.failed");
        Ok(())
    }

    #[test]
    fn a_duplicate_terminal_shaped_payload_is_never_forwarded_twice() -> Result<(), Box<dyn Error>>
    {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            write_sse(
                stream,
                &[
                    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}",
                    "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"length\"}]}",
                    "data: [DONE]",
                ],
            );
        })?;

        let events = run(&local(&endpoint), &sample_request());

        let [AdapterEvent::Completed(reason)] = events.as_slice() else {
            return Err(format!("unexpected event sequence: {}", events.len()).into());
        };
        assert!(matches!(reason, ChatFinishReason::Stop));
        Ok(())
    }

    #[test]
    fn local_request_keeps_its_wire_shape_and_sends_no_authorization() -> Result<(), Box<dyn Error>>
    {
        let (endpoint, captured) =
            spawn_capturing_fixture(|stream| write_sse(stream, &["data: [DONE]"]))?;

        run(&local(&endpoint), &sample_request());
        let request = captured.recv_timeout(StdDuration::from_secs(5))?;

        assert_eq!(request.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(request.header("authorization"), None);
        let body = request.json_body()?;
        assert_eq!(body["max_tokens"], 64);
        assert!(body.get("max_completion_tokens").is_none());
        assert_eq!(body["stream"], true);
        Ok(())
    }

    #[test]
    fn remote_request_uses_bearer_auth_and_the_current_output_limit_field(
    ) -> Result<(), Box<dyn Error>> {
        let (endpoint, captured) = spawn_capturing_fixture(|stream| {
            write_sse(
                stream,
                &[
                    "data: {\"choices\":[{\"delta\":{\"content\":\"Remote\"}}]}",
                    "data: [DONE]",
                ],
            );
        })?;
        let request = ChatRequest {
            model_key: "gpt-test".to_string(),
            messages: vec![
                ChatMessage {
                    role: ChatRole::System,
                    text: "be brief".to_string(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    text: "first".to_string(),
                },
                ChatMessage {
                    role: ChatRole::Assistant,
                    text: String::new(),
                },
                ChatMessage {
                    role: ChatRole::User,
                    text: "second".to_string(),
                },
            ],
        };

        let events = run(&remote(&endpoint, Some(b"sk-fixture"))?, &request);
        let observed = captured.recv_timeout(StdDuration::from_secs(5))?;

        assert!(matches!(
            events.as_slice(),
            [AdapterEvent::Delta(_), AdapterEvent::Completed(_)]
        ));
        assert_eq!(observed.request_line, "POST /v1/chat/completions HTTP/1.1");
        assert_eq!(observed.header("authorization"), Some("Bearer sk-fixture"));
        let body = observed.json_body()?;
        assert_eq!(body["model"], "gpt-test");
        assert_eq!(body["stream"], true);
        assert_eq!(body["max_completion_tokens"], 64);
        assert!(body.get("max_tokens").is_none());
        assert_eq!(
            body["messages"],
            serde_json::json!([
                {"role": "system", "content": "be brief"},
                {"role": "user", "content": "first"},
                {"role": "user", "content": "second"}
            ])
        );
        Ok(())
    }

    #[test]
    fn an_unbound_remote_target_sends_no_authorization() -> Result<(), Box<dyn Error>> {
        let (endpoint, captured) =
            spawn_capturing_fixture(|stream| write_sse(stream, &["data: [DONE]"]))?;

        run(&remote(&endpoint, None)?, &sample_request());

        assert_eq!(
            captured
                .recv_timeout(StdDuration::from_secs(5))?
                .header("authorization"),
            None
        );
        Ok(())
    }

    #[test]
    fn remote_statuses_are_normalized_without_leaking_the_response_body(
    ) -> Result<(), Box<dyn Error>> {
        for (status, expected_code) in [
            ("400 Bad Request", "provider.rejected"),
            ("401 Unauthorized", "provider.auth_failed"),
            ("403 Forbidden", "provider.auth_failed"),
            ("404 Not Found", "provider.rejected"),
            ("408 Request Timeout", "provider.timeout"),
            ("429 Too Many Requests", "provider.rate_limited"),
            ("500 Internal Server Error", "provider.unavailable"),
            ("503 Service Unavailable", "provider.unavailable"),
        ] {
            let (endpoint, _captured) = spawn_capturing_fixture(move |stream| {
                let body = format!("{{\"error\":{{\"message\":\"{SECRET_BODY_MARKER}\"}}}}");
                let _ = stream.write_all(
                    format!(
                        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
                        body.len()
                    )
                    .as_bytes(),
                );
            })?;

            let events = run(&remote(&endpoint, Some(b"sk-fixture"))?, &sample_request());

            let [AdapterEvent::Failed(error)] = events.as_slice() else {
                return Err(format!("{status}: unexpected events {events:?}").into());
            };
            assert_eq!(error.code, expected_code, "status {status}");
            assert!(!error.message.contains("VENDOR-DETAIL"));
            assert!(!error.message.contains("sk-"));
        }
        Ok(())
    }

    #[test]
    fn a_redirect_is_refused_and_never_followed() -> Result<(), Box<dyn Error>> {
        let redirect_target = UntouchedListener::bind()?;
        let location = format!(
            "http://127.0.0.1:{}/v1/chat/completions",
            redirect_target.port()?
        );
        let (endpoint, _captured) = spawn_capturing_fixture(move |stream| {
            let _ = stream.write_all(
                format!(
                    "HTTP/1.1 307 Temporary Redirect\r\nLocation: {location}\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                )
                .as_bytes(),
            );
        })?;

        let events = run(&remote(&endpoint, Some(b"sk-fixture"))?, &sample_request());

        assert_eq!(failed_code(&events)?, "provider.rejected");
        assert!(!redirect_target.was_contacted());
        Ok(())
    }

    #[test]
    fn an_environment_proxy_is_never_used() -> Result<(), Box<dyn Error>> {
        // Proxy selection is decided when the agent is built; asserting the
        // built configuration avoids mutating process-wide environment
        // variables that other tests running in parallel could observe.
        let agent = build_agent(Destination::Remote, Duration::from_secs(5));
        assert!(agent.config().proxy().is_none());
        assert_eq!(agent.config().max_redirects(), 0);
        assert!(!agent.config().http_status_as_error());

        let local_agent = build_agent(Destination::Local, Duration::from_secs(5));
        assert!(local_agent.config().proxy().is_none());
        assert_eq!(local_agent.config().max_redirects(), 0);
        Ok(())
    }

    #[test]
    fn an_error_payload_inside_a_stream_fails_the_run() -> Result<(), Box<dyn Error>> {
        let events_for = |target: fn(&str) -> Result<OpenAiCompatibleTarget, AppError>| {
            let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
                write_sse(
                    stream,
                    &[
                        "data: {\"choices\":[{\"delta\":{\"content\":\"Hi\"}}]}",
                        "data: {\"error\":{\"message\":\"VENDOR-DETAIL overloaded\"}}",
                        "data: [DONE]",
                    ],
                );
            })?;
            Ok::<_, Box<dyn Error>>(run(&target(&endpoint)?, &sample_request()))
        };

        let remote_events = events_for(|endpoint| remote(endpoint, None))?;
        let [AdapterEvent::Delta(_), AdapterEvent::Failed(remote_error)] = remote_events.as_slice()
        else {
            return Err(format!("unexpected remote events {remote_events:?}").into());
        };
        assert_eq!(remote_error.code, "provider.rejected");
        assert!(!remote_error.message.contains("VENDOR-DETAIL"));

        let local_events = events_for(|endpoint| Ok(local(endpoint)))?;
        let [AdapterEvent::Delta(_), AdapterEvent::Failed(local_error)] = local_events.as_slice()
        else {
            return Err(format!("unexpected local events {local_events:?}").into());
        };
        assert_eq!(local_error.code, "chat.failed");
        Ok(())
    }

    #[test]
    fn an_oversized_stream_line_fails_the_run() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\ndata: ",
            );
            let chunk = vec![b'a'; 64 * 1024];
            let mut written = 0;
            while written <= MAX_STREAM_LINE_BYTES + 1024 {
                if stream.write_all(&chunk).is_err() {
                    return;
                }
                written += chunk.len();
            }
            thread::sleep(StdDuration::from_millis(20));
        })?;

        let events = run(&remote(&endpoint, None)?, &sample_request());

        assert_eq!(failed_code(&events)?, "chat.failed");
        Ok(())
    }

    #[test]
    fn sse_variants_are_tolerated_and_refusal_text_is_forwarded() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n\
                  : keep-alive comment\r\n\r\n\
                  event: message\r\n\
                  data:{\"choices\":[{\"delta\":{\"content\":\"A\"}}]}\r\n\r\n\
                  data: {\"choices\":[]}\n\n\
                  data: {\"choices\":[{\"delta\":{\"refusal\":\"I can't\"},\"finish_reason\":\"content_filter\"}]}\n\n",
            );
            thread::sleep(StdDuration::from_millis(20));
        })?;

        let events = run(&remote(&endpoint, None)?, &sample_request());

        let [AdapterEvent::Delta(first), AdapterEvent::Delta(refusal), AdapterEvent::Completed(reason)] =
            events.as_slice()
        else {
            return Err(format!("unexpected events {events:?}").into());
        };
        assert_eq!(first, "A");
        assert_eq!(refusal, "I can't");
        assert!(matches!(reason, ChatFinishReason::Stop));
        Ok(())
    }

    #[test]
    fn a_remote_deadline_is_reported_as_a_timeout() -> Result<(), Box<dyn Error>> {
        let (endpoint, _captured) = spawn_capturing_fixture(|stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
            );
            thread::sleep(StdDuration::from_secs(3));
        })?;

        let started = Instant::now();
        let events = run_with_deadline(
            &remote(&endpoint, None)?,
            &sample_request(),
            Duration::from_millis(400),
        );

        assert_eq!(failed_code(&events)?, "provider.timeout");
        assert!(started.elapsed() < StdDuration::from_secs(3));
        Ok(())
    }

    #[test]
    fn an_unreachable_remote_endpoint_is_unavailable() -> Result<(), Box<dyn Error>> {
        let events = run(&remote("http://127.0.0.1:1", None)?, &sample_request());
        assert_eq!(failed_code(&events)?, "provider.unavailable");
        Ok(())
    }

    #[test]
    fn cancellation_before_dispatch_sends_no_request_and_no_event() -> Result<(), Box<dyn Error>> {
        let listener = UntouchedListener::bind()?;
        let endpoint = format!("http://127.0.0.1:{}", listener.port()?);
        let (sender, receiver) = mpsc::channel();

        stream_completion(
            &remote(&endpoint, Some(b"sk-fixture"))?,
            &sample_request(),
            64,
            Duration::from_secs(5),
            &AtomicBool::new(true),
            &sender,
        );
        drop(sender);

        assert!(receiver.recv().is_err(), "no event may be sent");
        assert!(!listener.was_contacted());
        Ok(())
    }

    #[test]
    fn cancellation_mid_stream_closes_the_connection_at_the_next_chunk(
    ) -> Result<(), Box<dyn Error>> {
        let (closed_sender, closed_receiver) = mpsc::channel();
        let (endpoint, _captured) = spawn_capturing_fixture(move |stream| {
            let _ = stream.write_all(
                b"HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\n\r\n\
                  data: {\"choices\":[{\"delta\":{\"content\":\"one\"}}]}\n\n",
            );
            thread::sleep(StdDuration::from_millis(300));
            let _ =
                stream.write_all(b"data: {\"choices\":[{\"delta\":{\"content\":\"two\"}}]}\n\n");
            let _ = stream.set_read_timeout(Some(StdDuration::from_secs(3)));
            let mut buffer = [0_u8; 1];
            let closed = matches!(stream.read(&mut buffer), Ok(0));
            let _ = closed_sender.send(closed);
        })?;

        let cancel = Arc::new(AtomicBool::new(false));
        let reader_cancel = cancel.clone();
        let target = remote(&endpoint, None)?;
        let (sender, receiver) = mpsc::channel();
        let reader = thread::spawn(move || {
            stream_completion(
                &target,
                &sample_request(),
                64,
                Duration::from_secs(10),
                &reader_cancel,
                &sender,
            );
        });

        let first = receiver.recv_timeout(StdDuration::from_secs(5))?;
        assert!(matches!(first, AdapterEvent::Delta(ref text) if text == "one"));
        cancel.store(true, Ordering::SeqCst);

        assert!(
            closed_receiver.recv_timeout(StdDuration::from_secs(5))?,
            "the client must close the connection after cancellation"
        );
        reader.join().map_err(|_| "reader thread panicked")?;
        assert!(
            receiver.try_recv().is_err(),
            "nothing may be forwarded after cancellation"
        );
        Ok(())
    }

    #[test]
    fn bearer_tokens_are_redacted_and_validated() -> Result<(), Box<dyn Error>> {
        let token = BearerToken::from_secret(b"  sk-visible\n".to_vec())?;
        assert_eq!(format!("{token:?}"), "BearerToken(<redacted>)");
        let value = token.header_value().ok_or("header value")?;
        assert!(value.is_sensitive());

        for invalid in [
            b"".to_vec(),
            b"sk bad".to_vec(),
            vec![b'a'; MAX_BEARER_TOKEN_BYTES + 1],
        ] {
            assert!(BearerToken::from_secret(invalid).is_err());
        }

        let target = OpenAiCompatibleTarget::remote("https://api.example.com/v1", Some(token));
        assert!(!format!("{target:?}").contains("sk-visible"));
        Ok(())
    }

    /// Opt-in real HTTPS evidence (network required, no credential, no
    /// spend): an unauthenticated request to OpenAI's public endpoint must
    /// complete a real TLS handshake against the bundled roots and be
    /// normalized to `provider.auth_failed`. Run with
    /// `LATTICE_REMOTE_SMOKE=1 cargo test -p lattice-core real_remote -- --ignored`.
    #[test]
    #[ignore = "requires network; set LATTICE_REMOTE_SMOKE=1"]
    fn real_remote_unauthenticated_https_request_is_auth_failed() -> Result<(), Box<dyn Error>> {
        if std::env::var("LATTICE_REMOTE_SMOKE").as_deref() != Ok("1") {
            return Ok(());
        }
        let target = OpenAiCompatibleTarget::remote("https://api.openai.com/v1", None);
        let request = ChatRequest {
            model_key: "gpt-4o-mini".to_string(),
            messages: vec![ChatMessage {
                role: ChatRole::User,
                text: "Lattice unauthenticated TLS smoke".to_string(),
            }],
        };

        let events = run_with_deadline(&target, &request, Duration::from_secs(30));

        assert_eq!(failed_code(&events)?, "provider.auth_failed");
        Ok(())
    }

    /// Opt-in authenticated streaming evidence for a maintainer with a real
    /// account (bounded spend: one short prompt, `MAX_OUTPUT_TOKENS`-capped
    /// by the caller's 64-token limit here). Requires
    /// `LATTICE_REMOTE_SMOKE=1` plus `LATTICE_REMOTE_SMOKE_ENDPOINT`,
    /// `LATTICE_REMOTE_SMOKE_MODEL` and `LATTICE_REMOTE_SMOKE_API_KEY`; the
    /// key is only ever placed in the request header.
    #[test]
    #[ignore = "requires network and a real provider credential"]
    fn real_remote_authenticated_stream_completes() -> Result<(), Box<dyn Error>> {
        if std::env::var("LATTICE_REMOTE_SMOKE").as_deref() != Ok("1") {
            return Ok(());
        }
        let (Ok(endpoint), Ok(model), Ok(key)) = (
            std::env::var("LATTICE_REMOTE_SMOKE_ENDPOINT"),
            std::env::var("LATTICE_REMOTE_SMOKE_MODEL"),
            std::env::var("LATTICE_REMOTE_SMOKE_API_KEY"),
        ) else {
            return Err("set LATTICE_REMOTE_SMOKE_ENDPOINT/_MODEL/_API_KEY".into());
        };
        let endpoint = super::super::profiles::validate_endpoint(&endpoint)?;
        let target = OpenAiCompatibleTarget::remote(
            &endpoint,
            Some(BearerToken::from_secret(key.into_bytes())?),
        );
        let request = ChatRequest {
            model_key: model,
            messages: vec![ChatMessage {
                role: ChatRole::User,
                text: "Reply with the single word: ready".to_string(),
            }],
        };

        let events = run_with_deadline(&target, &request, Duration::from_secs(60));

        assert!(
            events
                .iter()
                .any(|event| matches!(event, AdapterEvent::Delta(_))),
            "expected at least one delta, got {events:?}"
        );
        assert!(matches!(events.last(), Some(AdapterEvent::Completed(_))));
        Ok(())
    }
}
