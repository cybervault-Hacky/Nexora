use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use tauri::ipc::Channel;
use tokio::sync::Notify;

use super::error::{InferenceError, InferenceResult};
use super::process::{scrub_runtime_secret, LOOPBACK_HOST};
use super::sse::{classify, LineBuffer, SseLine};
use super::{ChatTurn, GenerationEvent, GenerationOutcome};

/// Time allowed before the first token arrives.
///
/// This covers prompt prefill, which is CPU-bound on large contexts, so it
/// matches the runtime's own 600 s read/write timeout. The user can stop at any
/// moment, which is the intended escape hatch rather than a false failure.
const FIRST_TOKEN_TIMEOUT: Duration = Duration::from_secs(600);
/// Time allowed between tokens once generation has started.
const IDLE_TIMEOUT: Duration = Duration::from_secs(120);
/// Upper bound on a single reply, so a runaway model cannot exhaust memory.
const MAX_RESPONSE_CHARS: usize = 400_000;

const SYSTEM_PROMPT: &str = "You are Nexora, a local coding assistant running on the user's own machine. \
Answer concisely and accurately. Never claim that code was run or tested unless the user says it was. \
When you are unsure, say so. Reply in plain Markdown.";

#[derive(Debug, Deserialize)]
struct StreamChunk {
    #[serde(default)]
    choices: Vec<StreamChoice>,
}

#[derive(Debug, Deserialize)]
struct StreamChoice {
    #[serde(default)]
    delta: Option<StreamDelta>,
    #[serde(default)]
    finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
struct StreamDelta {
    #[serde(default)]
    content: Option<String>,
}

#[derive(Debug, Default, PartialEq, Eq)]
pub struct DeltaPiece {
    pub text: Option<String>,
    pub finish_reason: Option<String>,
}

/// Interprets one SSE `data:` payload, distinguishing mid-stream engine errors
/// and unparseable corrupted payloads from normal delta frames.
#[derive(Debug, PartialEq, Eq)]
pub enum StreamFrame {
    Delta(DeltaPiece),
    Error(String),
    Malformed,
}

/// Interprets one `data:` payload. Unknown or malformed JSON is ignored rather
/// than aborting a reply that is otherwise healthy.
pub fn extract_delta(payload: &str) -> DeltaPiece {
    let Ok(chunk) = serde_json::from_str::<StreamChunk>(payload) else {
        return DeltaPiece::default();
    };

    let Some(choice) = chunk.choices.into_iter().next() else {
        return DeltaPiece::default();
    };

    let text = choice
        .delta
        .and_then(|delta| delta.content)
        .filter(|text| !text.is_empty());

    DeltaPiece {
        text,
        finish_reason: choice.finish_reason,
    }
}

/// Parses an SSE `data:` line, detecting corrupted non-JSON frames and
/// mid-stream OpenAI/llama-server error frames (`{"error":{"message":"..."}}`)
/// before falling back to delta extraction.
pub fn parse_stream_frame(payload: &str) -> StreamFrame {
    if serde_json::from_str::<serde_json::Value>(payload).is_err() {
        return StreamFrame::Malformed;
    }

    if let Some(err_msg) = error_message(payload) {
        return StreamFrame::Error(err_msg);
    }

    StreamFrame::Delta(extract_delta(payload))
}

/// Streams a chat completion from the loopback runtime with explicit timeouts.
#[allow(clippy::too_many_arguments)]
pub async fn stream_chat(
    port: u16,
    key: &str,
    model_id: &str,
    request_id: &str,
    messages: &[ChatTurn],
    max_tokens: u32,
    temperature: f32,
    on_event: &Channel<GenerationEvent>,
    cancel: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
    accepted: &AtomicBool,
) -> InferenceResult<GenerationOutcome> {
    stream_chat_with_timeouts(
        port,
        key,
        model_id,
        request_id,
        messages,
        max_tokens,
        temperature,
        on_event,
        cancel,
        cancelled,
        accepted,
        FIRST_TOKEN_TIMEOUT,
        IDLE_TOKEN_FALLBACK_TIMEOUT,
    )
    .await
}

const IDLE_TOKEN_FALLBACK_TIMEOUT: Duration = IDLE_TIMEOUT;

/// Streams a chat completion from the loopback runtime.
///
/// Cancellation is cooperative: the caller flips the flag and notifies, and the
/// next `select!` breaks out of the loop. The HTTP connection is dropped when
/// the function returns, which tells the server to stop.
#[allow(clippy::too_many_arguments)]
pub async fn stream_chat_with_timeouts(
    port: u16,
    key: &str,
    model_id: &str,
    request_id: &str,
    messages: &[ChatTurn],
    max_tokens: u32,
    temperature: f32,
    on_event: &Channel<GenerationEvent>,
    cancel: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
    accepted: &AtomicBool,
    first_token_timeout: Duration,
    idle_timeout: Duration,
) -> InferenceResult<GenerationOutcome> {
    let client = reqwest::Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()?;

    let mut turns = Vec::with_capacity(messages.len() + 1);
    turns.push(json!({ "role": "system", "content": SYSTEM_PROMPT }));

    for turn in messages {
        turns.push(json!({ "role": turn.role, "content": turn.content }));
    }

    let body = json!({
        "model": model_id,
        "stream": true,
        "messages": turns,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let request = client
        .post(format!("http://{LOOPBACK_HOST}:{port}/v1/chat/completions"))
        .bearer_auth(key)
        .json(&body)
        .send();

    // Waiting for the engine to accept the request must be as stoppable as the
    // stream itself, and must not be unbounded. Stop pressed before the headers
    // arrive now ends the request at once, and a silent engine times out. Nothing
    // was accepted in either case, so no usage unit is charged.
    let response = tokio::select! {
        biased;
        _ = cancel.notified() => return Ok(cancelled_outcome(request_id)),
        result = tokio::time::timeout(first_token_timeout, request) => match result {
            Ok(Ok(response)) => response,
            Ok(Err(error)) => {
                let clean = scrub_runtime_secret(&error.to_string(), Some(key));
                return Err(InferenceError::network(format!(
                    "The connection to the local engine failed: {clean}"
                )));
            }
            Err(_elapsed) => {
                return Err(InferenceError::timeout(
                    "The local engine did not accept the request in time. Nothing was sent; send the message again.",
                ))
            }
        },
    };

    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let raw_detail: String =
            error_message(&body).unwrap_or_else(|| body.chars().take(300).collect());
        let detail = scrub_runtime_secret(&raw_detail, Some(key));

        return Err(InferenceError::new(
            status_error_code(status.as_u16()),
            format!("The local engine rejected the request ({status}): {detail}"),
        ));
    }

    if cancelled.load(Ordering::SeqCst) {
        return Ok(cancelled_outcome(request_id));
    }

    // The engine has taken the request and is streaming. Everything before
    // this point is a failure that costs the user nothing; everything after it
    // is work the model really did.
    accepted.store(true, Ordering::SeqCst);

    let _ = on_event.send(GenerationEvent::Started {
        request_id: request_id.to_string(),
    });

    let mut stream = response.bytes_stream();
    let mut lines = LineBuffer::new();
    let mut produced = 0usize;
    let mut finish_reason: Option<String> = None;
    let mut finished = false;

    loop {
        if cancelled.load(Ordering::SeqCst) {
            return Ok(cancelled_outcome(request_id));
        }

        let wait = if produced == 0 {
            first_token_timeout
        } else {
            idle_timeout
        };

        let step = tokio::select! {
            biased;
            _ = cancel.notified() => None,
            result = tokio::time::timeout(wait, stream.next()) => Some(result),
        };

        let bytes = match step {
            None => return Ok(cancelled_outcome(request_id)),
            Some(Err(_elapsed)) => {
                return Err(InferenceError::timeout(
                    "The model stopped responding. The request was abandoned; the conversation is unchanged.",
                ))
            }
            Some(Ok(None)) => {
                if cancelled.load(Ordering::SeqCst) {
                    return Ok(cancelled_outcome(request_id));
                }
                if let Some(trailing) = lines.finish() {
                    process_sse_line(
                        &trailing,
                        key,
                        request_id,
                        on_event,
                        &cancelled,
                        &mut produced,
                        &mut finish_reason,
                        &mut finished,
                    )?;
                }
                break;
            }
            Some(Ok(Some(Err(error)))) => {
                let clean = scrub_runtime_secret(&error.to_string(), Some(key));
                return Err(InferenceError::network(format!(
                    "The connection to the local engine failed: {clean}"
                )));
            }
            Some(Ok(Some(Ok(bytes)))) => bytes,
        };

        let batch = lines.push(&bytes);
        if lines.take_overflow() {
            return Err(InferenceError::protocol(
                "The local engine sent an oversized malformed stream frame.",
            ));
        }

        for line in batch {
            if cancelled.load(Ordering::SeqCst) {
                return Ok(cancelled_outcome(request_id));
            }

            process_sse_line(
                &line,
                key,
                request_id,
                on_event,
                &cancelled,
                &mut produced,
                &mut finish_reason,
                &mut finished,
            )?;

            if finished {
                break;
            }
        }

        if finished {
            break;
        }
    }

    if cancelled.load(Ordering::SeqCst) {
        return Ok(cancelled_outcome(request_id));
    }

    // A stream can close without `[DONE]`: the process can exit, the socket can
    // be cut, or the runtime can fail mid-reply. That used to look like a
    // short but successful answer. It is not one.
    if !stream_completed(finished, finish_reason.as_deref()) {
        return Err(InferenceError::network(
            "The local engine ended the reply before reporting it finished. The answer may be incomplete; send the message again.",
        ));
    }

    Ok(GenerationOutcome {
        request_id: request_id.to_string(),
        finish_reason,
        cancelled: false,
    })
}

#[allow(clippy::too_many_arguments)]
fn process_sse_line(
    line: &str,
    key: &str,
    request_id: &str,
    on_event: &Channel<GenerationEvent>,
    cancelled: &AtomicBool,
    produced: &mut usize,
    finish_reason: &mut Option<String>,
    finished: &mut bool,
) -> InferenceResult<()> {
    match classify(line) {
        SseLine::Ignore => {}
        SseLine::Done => {
            *finished = true;
        }
        SseLine::Data(payload) => match parse_stream_frame(&payload) {
            StreamFrame::Malformed => {
                return Err(InferenceError::protocol(
                    "The local engine sent a malformed stream frame.",
                ));
            }
            StreamFrame::Error(err_msg) => {
                let clean = scrub_runtime_secret(&err_msg, Some(key));
                return Err(InferenceError::generation_failed(format!(
                    "The local engine reported an error during generation: {clean}"
                )));
            }
            StreamFrame::Delta(piece) => {
                if let Some(text) = piece.text {
                    if cancelled.load(Ordering::SeqCst) {
                        return Ok(());
                    }

                    *produced += text.chars().count();

                    if *produced > MAX_RESPONSE_CHARS {
                        return Err(InferenceError::protocol(
                            "The model produced more output than Nexora accepts.",
                        ));
                    }

                    let _ = on_event.send(GenerationEvent::Delta {
                        request_id: request_id.to_string(),
                        text,
                    });
                }

                if piece.finish_reason.is_some() {
                    *finish_reason = piece.finish_reason;
                }
            }
        },
    }

    Ok(())
}

/// Whether the stream ended in a way that proves the model finished.
///
/// Either the `[DONE]` sentinel arrived, or the last JSON chunk carried a
/// `finish_reason`. Anything else is a truncated reply and must not be shown as
/// a complete one.
pub fn stream_completed(done_sentinel: bool, finish_reason: Option<&str>) -> bool {
    done_sentinel || finish_reason.is_some_and(|reason| !reason.trim().is_empty())
}

/// Builds the "user stopped it" outcome.
///
/// Cancellation needs no slot cleanup: llama.cpp notices the dropped connection
/// (`req.should_stop`) and cancels the task itself. `POST /slots/0?action=erase`
/// is **not** available here — b11714 rejects every slot action with 501 unless
/// the server was started with `--slot-save-path` (`server-context.cpp`,
/// `post_slots`), and Nexora deliberately does not enable slot files.
fn cancelled_outcome(request_id: &str) -> GenerationOutcome {
    GenerationOutcome {
        request_id: request_id.to_string(),
        finish_reason: Some("stopped".to_string()),
        cancelled: true,
    }
}

/// Maps an HTTP rejection to a stable UI-facing code.
///
/// The runtime answers 401 for a bad session key, 503 while the model is still
/// loading, and 400 for a malformed request. Treating all of them as one
/// generic failure hid the real cause from the user.
pub fn status_error_code(status: u16) -> &'static str {
    match status {
        401 | 403 => "auth_failed",
        503 => "not_ready",
        400 | 404 | 422 => "invalid_request",
        _ => "engine_rejected",
    }
}

/// Pulls `error.message` out of the runtime's OpenAI-compatible error body.
///
/// The runtime (like the OpenAI API it mimics) puts a human message in
/// `error.message`; `tools/server/server-http.cpp` defines the shape. The
/// fixtures below are transcribed from that contract, not captured here.
pub fn error_message(body: &str) -> Option<String> {
    #[derive(Deserialize)]
    struct Envelope {
        error: ErrorBody,
    }

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum ErrorBody {
        Object { message: String },
        Plain(String),
    }

    serde_json::from_str::<Envelope>(body)
        .ok()
        .map(|envelope| match envelope.error {
            ErrorBody::Object { message } => message,
            ErrorBody::Plain(message) => message,
        })
        .filter(|message| !message.trim().is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_streamed_content() {
        let piece = extract_delta(r#"{"choices":[{"delta":{"content":"hello"}}]}"#);

        assert_eq!(piece.text.as_deref(), Some("hello"));
        assert_eq!(piece.finish_reason, None);
    }

    #[test]
    fn captures_the_finish_reason() {
        let piece = extract_delta(r#"{"choices":[{"delta":{},"finish_reason":"stop"}]}"#);

        assert_eq!(piece.text, None);
        assert_eq!(piece.finish_reason.as_deref(), Some("stop"));
    }

    #[test]
    fn ignores_role_only_chunks() {
        let piece = extract_delta(r#"{"choices":[{"delta":{"role":"assistant"}}]}"#);

        assert_eq!(piece, DeltaPiece::default());
    }

    #[test]
    fn ignores_empty_content() {
        let piece = extract_delta(r#"{"choices":[{"delta":{"content":""}}]}"#);

        assert_eq!(piece.text, None);
    }

    #[test]
    fn tolerates_malformed_json() {
        assert_eq!(extract_delta("{not json"), DeltaPiece::default());
    }

    #[test]
    fn tolerates_an_empty_choice_list() {
        assert_eq!(extract_delta(r#"{"choices":[]}"#), DeltaPiece::default());
    }

    /// Error envelopes shaped like the runtime's documented contract.
    const ENVELOPE_401: &str =
        r#"{"error":{"message":"Invalid API Key","type":"authentication_error","code":401}}"#;
    const ENVELOPE_400: &str = r#"{"error":{"code":400,"message":"'messages' is required","type":"invalid_request_error"}}"#;

    #[test]
    fn maps_http_rejections_to_specific_codes() {
        assert_eq!(status_error_code(401), "auth_failed");
        assert_eq!(status_error_code(403), "auth_failed");
        assert_eq!(status_error_code(503), "not_ready");
        assert_eq!(status_error_code(400), "invalid_request");
        assert_eq!(status_error_code(500), "engine_rejected");
    }

    #[test]
    fn reads_the_runtime_error_message() {
        assert_eq!(
            error_message(ENVELOPE_401).as_deref(),
            Some("Invalid API Key")
        );
        assert_eq!(
            error_message(ENVELOPE_400).as_deref(),
            Some("'messages' is required")
        );
        assert_eq!(
            parse_stream_frame(r#"{"error":{"message":"KV cache out of memory"}}"#),
            StreamFrame::Error("KV cache out of memory".to_string())
        );
    }

    #[test]
    fn falls_back_when_the_body_is_not_json() {
        assert_eq!(error_message("<html>nope</html>"), None);
        assert_eq!(error_message(r#"{"error":{"message":"  "}}"#), None);
    }

    #[test]
    fn the_401_body_is_not_reported_as_a_model_failure() {
        // Regression: every rejection used to be "load_failed", which hid an
        // auth problem behind a model message.
        assert_ne!(status_error_code(401), "load_failed");
    }

    #[test]
    fn a_done_sentinel_completes_the_stream() {
        assert!(stream_completed(true, None));
        assert!(stream_completed(true, Some("stop")));
    }

    #[test]
    fn a_finish_reason_completes_the_stream_without_the_sentinel() {
        assert!(stream_completed(false, Some("stop")));
        assert!(stream_completed(false, Some("length")));
    }

    #[test]
    fn a_stream_that_just_stops_is_not_complete() {
        // The runtime exited, or the connection dropped, between tokens.
        assert!(!stream_completed(false, None));
        assert!(!stream_completed(false, Some("   ")));
    }

    #[test]
    fn tolerates_missing_fields() {
        assert_eq!(extract_delta("{}"), DeltaPiece::default());
        assert_eq!(parse_stream_frame("{not-json"), StreamFrame::Malformed);
    }

    #[tokio::test]
    async fn streams_split_utf8_multi_event_chunks_stops_immediately_on_cancel_and_rejects_malformed_frames(
    ) {
        use std::io::{Read as _, Write as _};
        use std::net::TcpListener;
        use std::sync::Mutex as StdMutex;
        use tauri::ipc::InvokeResponseBody;

        let make_channel = |cancel_after: Option<(usize, Arc<AtomicBool>)>| {
            let captured = Arc::new(StdMutex::new(Vec::<GenerationEvent>::new()));
            let sink = Arc::clone(&captured);
            let ch = Channel::new(move |body: InvokeResponseBody| {
                if let InvokeResponseBody::Json(raw) = body {
                    if let Ok(ev) = serde_json::from_str::<GenerationEvent>(&raw) {
                        let mut guard = sink.lock().expect("lock");
                        guard.push(ev);
                        let deltas = guard
                            .iter()
                            .filter(|e| matches!(e, GenerationEvent::Delta { .. }))
                            .count();
                        if let Some((limit, ref flag)) = cancel_after {
                            if deltas >= limit {
                                flag.store(true, Ordering::SeqCst);
                            }
                        }
                    }
                }
                Ok(())
            });
            (ch, captured)
        };

        // 1. Byte-split 4-byte UTF-8 emoji + CJK + multi-event single chunk + trailing non-newline finish.
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind");
        let port = listener.local_addr().expect("addr").port();
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().expect("accept");
            let mut req = [0u8; 2048];
            let _ = s.read(&mut req);
            let sse_body = concat!(
                "data: {\"choices\":[{\"delta\":{\"content\":\"Hello 🦀 \"}}]}\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"日本語\"}}]}\ndata: {\"choices\":[{\"delta\":{\"content\":\"!\"}}]}\n",
                "data: {\"choices\":[{\"delta\":{},\"finish_reason\":\"stop\"}]}"
            );
            let raw = sse_body.as_bytes();
            let header =
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n";
            let _ = s.write_all(header.as_bytes());
            // Split right inside the 4-byte crab emoji `🦀` (`F0 9F A6 80`)!
            let emoji_pos = sse_body.find('🦀').expect("emoji");
            let _ = s.write_all(&raw[..emoji_pos + 2]);
            let _ = s.flush();
            std::thread::sleep(Duration::from_millis(15));
            let _ = s.write_all(&raw[emoji_pos + 2..]);
            let _ = s.flush();
        });

        let (ch, events) = make_channel(None);
        let outcome = stream_chat_with_timeouts(
            port,
            "super-secret-session-key",
            "nexora-mini",
            "req-utf8",
            &[ChatTurn {
                role: "user".into(),
                content: "hi".into(),
            }],
            64,
            0.2,
            &ch,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
            &AtomicBool::new(false),
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .await
        .expect("clean utf8 stream");
        assert!(!outcome.cancelled);
        assert_eq!(outcome.finish_reason.as_deref(), Some("stop"));
        let joined: String = events
            .lock()
            .expect("lock")
            .iter()
            .filter_map(|e| match e {
                GenerationEvent::Delta { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(joined, "Hello 🦀 日本語!");

        // 2. Single chunk packed with 5 delta lines stops forwarding immediately after token #2 cancels!
        let listener2 = TcpListener::bind("127.0.0.1:0").expect("bind 2");
        let port2 = listener2.local_addr().expect("addr 2").port();
        std::thread::spawn(move || {
            let (mut s, _) = listener2.accept().expect("accept 2");
            let mut req = [0u8; 2048];
            let _ = s.read(&mut req);
            let packed = concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"one-\"}}]}\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"two\"}}]}\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"-NEVER-3\"}}]}\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"-NEVER-4\"}}]}\n",
                "data: [DONE]\n\n"
            );
            let _ = s.write_all(packed.as_bytes());
        });
        let cancel_flag = Arc::new(AtomicBool::new(false));
        let (ch2, events2) = make_channel(Some((2, Arc::clone(&cancel_flag))));
        let out2 = stream_chat_with_timeouts(
            port2,
            "super-secret-session-key",
            "nexora-mini",
            "req-packed-cancel",
            &[ChatTurn {
                role: "user".into(),
                content: "hi".into(),
            }],
            64,
            0.2,
            &ch2,
            Arc::new(Notify::new()),
            cancel_flag,
            &AtomicBool::new(false),
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .await
        .expect("cancelled mid-batch");
        assert!(out2.cancelled);
        let texts2: Vec<String> = events2
            .lock()
            .expect("lock")
            .iter()
            .filter_map(|e| match e {
                GenerationEvent::Delta { text, .. } => Some(text.clone()),
                _ => None,
            })
            .collect();
        assert_eq!(texts2, vec!["one-".to_string(), "two".to_string()]);

        // 3. Corrupted non-JSON SSE `data:` frame is rejected even if followed by `data: [DONE]`.
        let listener3 = TcpListener::bind("127.0.0.1:0").expect("bind 3");
        let port3 = listener3.local_addr().expect("addr 3").port();
        std::thread::spawn(move || {
            let (mut s, _) = listener3.accept().expect("accept 3");
            let mut req = [0u8; 2048];
            let _ = s.read(&mut req);
            let corrupt = concat!(
                "HTTP/1.1 200 OK\r\nContent-Type: text/event-stream\r\nConnection: close\r\n\r\n",
                "data: {\"choices\":[{\"delta\":{\"content\":\"half\"}}]}\n",
                "data: {CORRUPTED_NOT_JSON\n",
                "data: [DONE]\n\n"
            );
            let _ = s.write_all(corrupt.as_bytes());
        });
        let (ch3, _) = make_channel(None);
        let err3 = stream_chat_with_timeouts(
            port3,
            "super-secret-session-key",
            "nexora-mini",
            "req-corrupt",
            &[ChatTurn {
                role: "user".into(),
                content: "hi".into(),
            }],
            64,
            0.2,
            &ch3,
            Arc::new(Notify::new()),
            Arc::new(AtomicBool::new(false)),
            &AtomicBool::new(false),
            Duration::from_secs(2),
            Duration::from_secs(2),
        )
        .await
        .expect_err("corrupt frame must fail");
        assert_eq!(err3.code, "protocol");
    }
}
