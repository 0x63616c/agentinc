//! Incremental server-sent event parsing shared by streaming model transports.
use turnkeel::ModelError;

/// Collects a bounded response body and yields complete `data:` payloads.
pub struct SseReader {
    buffer: Vec<u8>,
    consumed: usize,
    limit: usize,
}
impl SseReader {
    pub fn new(limit: usize) -> Self {
        Self {
            buffer: Vec::new(),
            consumed: 0,
            limit,
        }
    }
    /// Feed one chunk and return every complete event payload it completed.
    pub fn push(&mut self, chunk: &[u8]) -> Result<Vec<String>, ModelError> {
        if self.buffer.len() - self.consumed + chunk.len() > self.limit {
            return Err(ModelError::fatal(
                "Model response exceeded the supported size.",
            ));
        }
        self.buffer.extend_from_slice(chunk);
        let mut events = Vec::new();
        loop {
            let pending = &self.buffer[self.consumed..];
            let Some(end) = find_boundary(pending) else {
                break;
            };
            let (event, skip) = end;
            let text = String::from_utf8_lossy(&pending[..event]).replace("\r\n", "\n");
            self.consumed += event + skip;
            let data = text
                .lines()
                .filter_map(|line| line.strip_prefix("data:").map(str::trim_start))
                .collect::<Vec<_>>()
                .join("\n");
            if !data.is_empty() && data != "[DONE]" {
                events.push(data);
            }
        }
        if self.consumed > 64 * 1024 {
            self.buffer.drain(..self.consumed);
            self.consumed = 0;
        }
        Ok(events)
    }
}
fn find_boundary(bytes: &[u8]) -> Option<(usize, usize)> {
    let mut i = 0;
    while i + 1 < bytes.len() {
        if bytes[i] == b'\n' && bytes[i + 1] == b'\n' {
            return Some((i, 2));
        }
        if i + 3 < bytes.len() && &bytes[i..i + 4] == b"\r\n\r\n" {
            return Some((i, 4));
        }
        i += 1;
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn splits_events_across_chunks_and_bounds_size() {
        let mut reader = SseReader::new(64);
        assert!(
            reader
                .push(b"event: a\ndata: {\"x\":1}\n\ndata: [DONE]\n\ndata: pa")
                .unwrap()
                == vec!["{\"x\":1}"]
        );
        assert_eq!(reader.push(b"rt\r\n\r\n").unwrap(), vec!["part"]);
        assert!(reader.push(&[b'x'; 65]).is_err());
    }
}
