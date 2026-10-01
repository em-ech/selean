//! Server-sent event plumbing shared by the provider stream parsers.

use crate::chat::ChatEvent;

/// Turns the raw bytes of a provider's streaming response into [`ChatEvent`]
/// values. Synchronous so it can be tested without a network.
pub trait EventParser: Send {
    /// Feeds one chunk of the response body and returns the events that can
    /// be emitted from the data received so far.
    fn feed(&mut self, chunk: &[u8]) -> Vec<ChatEvent>;

    /// Called once when the response body ends. Returns any events that were
    /// still pending.
    fn finish(&mut self) -> Vec<ChatEvent>;
}

/// Splits a streamed SSE body into the payloads of its `data:` lines.
///
/// Buffers bytes rather than text, so a multi-byte character split across
/// two chunks is decoded intact.
#[derive(Debug, Default)]
pub struct SseDataLines {
    buffer: Vec<u8>,
}

impl SseDataLines {
    /// Creates an empty buffer.
    pub fn new() -> Self {
        Self::default()
    }

    /// Appends a chunk and returns the payload of every complete, non-empty
    /// `data:` line now available.
    pub fn feed(&mut self, chunk: &[u8]) -> Vec<String> {
        self.buffer.extend_from_slice(chunk);

        let mut payloads = Vec::new();
        while let Some(newline_pos) = self.buffer.iter().position(|byte| *byte == b'\n') {
            let line: Vec<u8> = self.buffer.drain(..=newline_pos).collect();
            payloads.extend(data_payload(&line));
        }
        payloads
    }

    /// Returns the payload of a final `data:` line that was not terminated by
    /// a newline, if the body ended on one.
    pub fn finish(&mut self) -> Option<String> {
        let line = std::mem::take(&mut self.buffer);
        data_payload(&line)
    }
}

/// Extracts the payload of one SSE line, or `None` for comments, other
/// fields and empty data.
fn data_payload(line: &[u8]) -> Option<String> {
    let line = String::from_utf8_lossy(line);
    let data = line.strip_prefix("data:")?.trim();
    (!data.is_empty()).then(|| data.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn yields_data_payloads_and_skips_other_lines() {
        let mut lines = SseDataLines::new();
        let payloads =
            lines.feed(b"event: ping\ndata: {\"a\":1}\n\n: comment\ndata:[DONE]\r\ndata: \n");
        assert_eq!(
            payloads,
            vec!["{\"a\":1}".to_string(), "[DONE]".to_string()]
        );
    }

    #[test]
    fn holds_a_partial_line_until_its_newline_arrives() {
        let mut lines = SseDataLines::new();
        assert!(lines.feed(b"data: hel").is_empty());
        assert_eq!(lines.feed(b"lo\n"), vec!["hello".to_string()]);
    }

    #[test]
    fn multi_byte_character_split_across_chunks_is_decoded_intact() {
        let bytes = "data: caf\u{e9}\n".as_bytes();
        let split = bytes.len() - 2; // inside the two-byte "é"
        let mut lines = SseDataLines::new();
        assert!(lines.feed(&bytes[..split]).is_empty());
        assert_eq!(lines.feed(&bytes[split..]), vec!["caf\u{e9}".to_string()]);
    }

    #[test]
    fn finish_returns_an_unterminated_final_line() {
        let mut lines = SseDataLines::new();
        assert!(lines.feed(b"data: [DONE]").is_empty());
        assert_eq!(lines.finish(), Some("[DONE]".to_string()));
        assert_eq!(lines.finish(), None);
    }
}
