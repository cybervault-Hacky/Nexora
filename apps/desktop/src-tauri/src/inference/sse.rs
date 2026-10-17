//! Minimal server-sent-events handling for the loopback runtime.
//!
//! The runtime emits one JSON object per `data:` line. This module only deals
//! with framing; JSON interpretation lives in `client`.

/// One classified line from the event stream.
#[derive(Debug, PartialEq, Eq)]
pub enum SseLine {
    /// A `data:` payload, trimmed.
    Data(String),
    /// The `[DONE]` sentinel.
    Done,
    /// A comment, heartbeat, blank line, or an event field we do not use.
    Ignore,
}

/// Accumulates bytes and yields complete lines.
///
/// Bytes are buffered rather than decoded eagerly so a multi-byte character
/// split across two network chunks is never corrupted.
#[derive(Default)]
pub struct LineBuffer {
    pending: Vec<u8>,
}

/// Hard cap for a single unterminated line, so a misbehaving server cannot
/// grow memory without bound.
const MAX_PENDING_BYTES: usize = 1_048_576;

impl LineBuffer {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, chunk: &[u8]) -> Vec<String> {
        self.pending.extend_from_slice(chunk);

        let mut lines = Vec::new();

        while let Some(position) = self.pending.iter().position(|byte| *byte == b'\n') {
            let mut raw: Vec<u8> = self.pending.drain(..=position).collect();
            raw.pop();

            if raw.last() == Some(&b'\r') {
                raw.pop();
            }

            lines.push(String::from_utf8_lossy(&raw).into_owned());
        }

        if self.pending.len() > MAX_PENDING_BYTES {
            self.pending.clear();
        }

        lines
    }

    /// Test-facing probe: how many bytes are waiting for a newline.
    #[cfg(test)]
    pub fn buffered_bytes(&self) -> usize {
        self.pending.len()
    }
}

/// Classifies one line of the stream.
pub fn classify(line: &str) -> SseLine {
    let line = line.trim_end();

    if line.is_empty() || line.starts_with(':') {
        return SseLine::Ignore;
    }

    let Some(payload) = line.strip_prefix("data:") else {
        return SseLine::Ignore;
    };

    let payload = payload.trim();

    if payload == "[DONE]" {
        SseLine::Done
    } else if payload.is_empty() {
        SseLine::Ignore
    } else {
        SseLine::Data(payload.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_only_complete_lines() {
        let mut buffer = LineBuffer::new();

        assert!(buffer.push(b"data: one\npartial").is_empty());
        assert_eq!(buffer.buffered_bytes(), 7);

        let lines = buffer.push(b" two\n");

        assert_eq!(lines, vec!["partial two".to_string()]);
        assert_eq!(buffer.buffered_bytes(), 0);
    }

    #[test]
    fn handles_crlf_and_blank_lines() {
        let mut buffer = LineBuffer::new();

        assert_eq!(
            buffer.push(b"data: a\r\n\r\n: ping\r\n"),
            vec!["data: a", "", ": ping"]
        );
    }

    #[test]
    fn keeps_multibyte_characters_intact_across_chunks() {
        let text = "data: {\"t\":\"héllo\"}\n";
        let bytes = text.as_bytes();
        let split = text.find('é').expect("accent present") + 1;

        let mut buffer = LineBuffer::new();
        assert!(buffer.push(&bytes[..split]).is_empty());

        let lines = buffer.push(&bytes[split..]);

        assert_eq!(lines, vec!["data: {\"t\":\"héllo\"}".to_string()]);
    }

    #[test]
    fn drops_oversized_unterminated_input() {
        let mut buffer = LineBuffer::new();
        let huge = vec![b'x'; MAX_PENDING_BYTES + 16];

        let lines = buffer.push(&huge);

        assert!(lines.is_empty());
        assert_eq!(buffer.buffered_bytes(), 0);
    }

    #[test]
    fn classifies_data_done_and_noise() {
        assert_eq!(
            classify("data: {\"a\":1}"),
            SseLine::Data("{\"a\":1}".into())
        );
        assert_eq!(classify("data: [DONE]"), SseLine::Done);
        assert_eq!(classify(""), SseLine::Ignore);
        assert_eq!(classify(": keep-alive"), SseLine::Ignore);
        assert_eq!(classify("event: message"), SseLine::Ignore);
        assert_eq!(classify("data:"), SseLine::Ignore);
    }
}
