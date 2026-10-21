use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use futures_util::StreamExt;
use serde::Deserialize;
use serde_json::json;
use tauri::ipc::Channel;
use tokio::sync::Notify;

use super::error::{InferenceError, InferenceResult};
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

/// Streams a chat completion from the loopback runtime.
///
/// Cancellation is cooperative: the caller flips the flag and notifies, and the
/// next `select!` breaks out of the loop. The HTTP connection is dropped when
/// the function returns, which tells the server to stop.
#[allow(clippy::too_many_arguments)]
pub async fn stream_chat(
    port: u16,
    key: &str,
    request_id: &str,
    messages: &[ChatTurn],
    max_tokens: u32,
    temperature: f32,
    on_event: &Channel<GenerationEvent>,
    cancel: Arc<Notify>,
    cancelled: Arc<AtomicBool>,
    accepted: &AtomicBool,
) -> InferenceResult<GenerationOutcome> {
    let client = reqwest::Client::builder().no_proxy().build()?;

    let mut turns = Vec::with_capacity(messages.len() + 1);
    turns.push(json!({ "role": "system", "content": SYSTEM_PROMPT }));

    for turn in messages {
        turns.push(json!({ "role": turn.role, "content": turn.content }));
    }

    let body = json!({
        "model": "local",
        "stream": true,
        "messages": turns,
        "max_tokens": max_tokens,
        "temperature": temperature,
    });

    let response = client
        .post(format!("http://127.0.0.1:{port}/v1/chat/completions"))
        .bearer_auth(key)
        .json(&body)
        .send()
        .await?;

    let status = response.status();

    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        let detail: String =
            error_message(&body).unwrap_or_else(|| body.chars().take(300).collect());

        return Err(InferenceError::new(
            status_error_code(status.as_u16()),
            format!("The local engine rejected the request ({status}): {detail}"),
        ));
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
            FIRST_TOKEN_TIMEOUT
        } else {
            IDLE_TIMEOUT
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
            Some(Ok(None)) => break,
            Some(Ok(Some(Err(error)))) => {
                return Err(InferenceError::network(format!(
                    "The connection to the local engine failed: {error}"
                )))
            }
            Some(Ok(Some(Ok(bytes)))) => bytes,
        };

        for line in lines.push(&bytes) {
            match classify(&line) {
                SseLine::Ignore => {}
                SseLine::Done => {
                    finished = true;
                    break;
                }
                SseLine::Data(payload) => {
                    let piece = extract_delta(&payload);

                    if let Some(text) = piece.text {
                        produced += text.chars().count();

                        if produced > MAX_RESPONSE_CHARS {
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
                        finish_reason = piece.finish_reason;
                    }
                }
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
    struct ErrorBody {
        message: String,
    }

    serde_json::from_str::<Envelope>(body)
        .ok()
        .map(|envelope| envelope.error.message)
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
    }
}
