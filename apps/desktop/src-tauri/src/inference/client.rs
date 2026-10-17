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
        let detail = response.text().await.unwrap_or_default();
        let snippet: String = detail.chars().take(300).collect();

        return Err(InferenceError::load_failed(format!(
            "The local engine rejected the request ({status}). {snippet}"
        )));
    }

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

    Ok(GenerationOutcome {
        request_id: request_id.to_string(),
        finish_reason,
        cancelled: false,
    })
}

fn cancelled_outcome(request_id: &str) -> GenerationOutcome {
    GenerationOutcome {
        request_id: request_id.to_string(),
        finish_reason: Some("stopped".to_string()),
        cancelled: true,
    }
}

/// Best-effort slot cleanup after a stop. Failure is ignored: the connection
/// drop is what actually stops generation.
pub async fn release_slot(port: u16, key: &str) {
    let Ok(client) = reqwest::Client::builder()
        .no_proxy()
        .timeout(Duration::from_secs(2))
        .build()
    else {
        return;
    };

    let _ = client
        .post(format!("http://127.0.0.1:{port}/slots/0?action=erase"))
        .bearer_auth(key)
        .send()
        .await;
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

    #[test]
    fn tolerates_missing_fields() {
        assert_eq!(extract_delta("{}"), DeltaPiece::default());
    }
}
