//! Buffer for accumulating SSE (Server-Sent Events) data and extracting complete events.
//!
//! SSE events are delimited by double newlines (`\n\n` or `\r\n\r\n`). TCP chunks may
//! arrive with events split across boundaries, so this buffer accumulates data until
//! complete events can be extracted. Used by every server-executed tool that
//! intercepts streaming Responses API output.

use bytes::{Bytes, BytesMut};

/// Buffer for accumulating SSE data and extracting complete events.
///
/// # Example
///
/// ```ignore
/// let mut buffer = SseBuffer::new();
///
/// // First chunk arrives with partial event
/// buffer.extend(b"data: {\"type\":");
/// assert!(buffer.extract_complete_events().is_empty());
///
/// // Second chunk completes the event
/// buffer.extend(b" \"test\"}\n\n");
/// let events = buffer.extract_complete_events();
/// assert_eq!(events.len(), 1);
/// ```
#[derive(Debug, Default)]
pub struct SseBuffer {
    buffer: BytesMut,
}

impl SseBuffer {
    /// Create a new empty SSE buffer.
    pub fn new() -> Self {
        Self {
            buffer: BytesMut::new(),
        }
    }

    /// Append data to the buffer.
    pub fn extend(&mut self, data: &[u8]) {
        self.buffer.extend_from_slice(data);
    }

    /// Extract all complete SSE events from the buffer.
    ///
    /// Returns a vector of complete events (each ending with `\n\n` or `\r\n\r\n`)
    /// and retains any incomplete data for the next call.
    pub fn extract_complete_events(&mut self) -> Vec<Bytes> {
        let mut events = Vec::new();

        while let Some(end_pos) = self.find_event_boundary() {
            let event = self.buffer.split_to(end_pos);
            events.push(event.freeze());
        }

        events
    }

    /// Find the position of the next event boundary (end of `\n\n` or `\r\n\r\n`).
    fn find_event_boundary(&self) -> Option<usize> {
        let bytes = &self.buffer[..];

        for i in 0..bytes.len().saturating_sub(1) {
            if bytes[i] == b'\n' && bytes[i + 1] == b'\n' {
                return Some(i + 2);
            }
        }

        for i in 0..bytes.len().saturating_sub(3) {
            if bytes[i] == b'\r'
                && bytes[i + 1] == b'\n'
                && bytes[i + 2] == b'\r'
                && bytes[i + 3] == b'\n'
            {
                return Some(i + 4);
            }
        }

        None
    }

    /// Get any remaining incomplete data in the buffer.
    pub fn take_remaining(&mut self) -> Bytes {
        self.buffer.split().freeze()
    }

    /// Check if the buffer is empty.
    pub fn is_empty(&self) -> bool {
        self.buffer.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_single_event() {
        let mut buf = SseBuffer::new();
        buf.extend(b"data: hello\n\n");
        let events = buf.extract_complete_events();
        assert_eq!(events.len(), 1);
        assert_eq!(&events[0][..], b"data: hello\n\n");
        assert!(buf.is_empty());
    }

    #[test]
    fn extracts_event_split_across_chunks() {
        let mut buf = SseBuffer::new();
        buf.extend(b"data: hel");
        assert!(buf.extract_complete_events().is_empty());
        buf.extend(b"lo\n\n");
        let events = buf.extract_complete_events();
        assert_eq!(events.len(), 1);
        assert_eq!(&events[0][..], b"data: hello\n\n");
    }

    #[test]
    fn extracts_multiple_events_in_one_call() {
        let mut buf = SseBuffer::new();
        buf.extend(b"data: one\n\ndata: two\n\n");
        let events = buf.extract_complete_events();
        assert_eq!(events.len(), 2);
        assert_eq!(&events[0][..], b"data: one\n\n");
        assert_eq!(&events[1][..], b"data: two\n\n");
    }

    #[test]
    fn handles_crlf_delimiters() {
        let mut buf = SseBuffer::new();
        buf.extend(b"data: win\r\n\r\n");
        let events = buf.extract_complete_events();
        assert_eq!(events.len(), 1);
        assert_eq!(&events[0][..], b"data: win\r\n\r\n");
    }

    #[test]
    fn take_remaining_returns_incomplete_data() {
        let mut buf = SseBuffer::new();
        buf.extend(b"partial");
        let remaining = buf.take_remaining();
        assert_eq!(&remaining[..], b"partial");
        assert!(buf.is_empty());
    }
}
