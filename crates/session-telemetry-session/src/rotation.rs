use crate::chunk::{Chunk, ChunkWriter};
use session_telemetry_protocol::Event;

/// When to rotate to a new chunk — plan.md section 8.5: "The writer rotates after a configured
/// duration or encoded size." Either threshold being crossed triggers a rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationPolicy {
    pub max_encoded_size_bytes: usize,
    pub max_duration_ms: f64,
}

/// Wraps `ChunkWriter` with rotation, so a long QA capture produces many independently
/// decodable chunks instead of one `ChunkWriter` accumulating every event in memory for the
/// whole session — the same unbounded-growth problem the JS library's event buffer had before
/// its own sliding-window fix, on the Rust side of the pipeline instead.
#[derive(Debug)]
pub struct RotatingChunkWriter {
    policy: RotationPolicy,
    current: ChunkWriter,
    current_encoded_size_bytes: usize,
    current_start_timestamp: Option<f64>,
    sealed: Vec<Chunk>,
}

impl RotatingChunkWriter {
    pub fn new(policy: RotationPolicy) -> Self {
        Self {
            policy,
            current: ChunkWriter::new(),
            current_encoded_size_bytes: 0,
            current_start_timestamp: None,
            sealed: Vec::new(),
        }
    }

    pub fn push(&mut self, event: Event) {
        // Encoded size of just this one event — an incrementally maintained running total,
        // not re-serializing the whole accumulated chunk on every push.
        let event_size_bytes = serde_json::to_vec(&event).map_or(0, |bytes| bytes.len());
        let timestamp = event.timestamp();
        let start_timestamp = *self.current_start_timestamp.get_or_insert(timestamp);

        self.current.push(event);
        self.current_encoded_size_bytes += event_size_bytes;

        let duration_ms = timestamp - start_timestamp;
        let exceeds_size = self.current_encoded_size_bytes >= self.policy.max_encoded_size_bytes;
        let exceeds_duration = duration_ms >= self.policy.max_duration_ms;

        if exceeds_size || exceeds_duration {
            self.rotate();
        }
    }

    fn rotate(&mut self) {
        let writer = std::mem::take(&mut self.current);
        if let Some(chunk) = writer.seal() {
            self.sealed.push(chunk);
        }
        self.current_encoded_size_bytes = 0;
        self.current_start_timestamp = None;
    }

    /// Seals whatever's left accumulated (e.g. at session end, where the last chunk usually
    /// hasn't hit a rotation threshold) and returns every sealed chunk in order.
    pub fn finish(mut self) -> Vec<Chunk> {
        self.rotate();
        self.sealed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::RemoteInputEvent;

    fn event(sequence: u64, timestamp: f64) -> Event {
        Event::RemoteInput(RemoteInputEvent {
            sequence,
            timestamp,
            key: "right".to_string(),
        })
    }

    fn chunk_sizes(chunks: &[Chunk]) -> Vec<usize> {
        chunks.iter().map(|chunk| chunk.events.len()).collect()
    }

    #[test]
    fn rotates_by_encoded_size() {
        let one_event_size = serde_json::to_vec(&event(0, 0.0)).unwrap().len();
        let mut writer = RotatingChunkWriter::new(RotationPolicy {
            max_encoded_size_bytes: one_event_size * 2 - 1,
            max_duration_ms: f64::INFINITY,
        });

        for sequence in 0..5 {
            writer.push(event(sequence, sequence as f64));
        }

        assert_eq!(chunk_sizes(&writer.finish()), vec![2, 2, 1]);
    }

    #[test]
    fn rotates_by_duration() {
        let mut writer = RotatingChunkWriter::new(RotationPolicy {
            max_encoded_size_bytes: usize::MAX,
            max_duration_ms: 10.0,
        });

        for (sequence, timestamp) in [0.0, 5.0, 12.0, 15.0, 25.0].into_iter().enumerate() {
            writer.push(event(sequence as u64, timestamp));
        }

        assert_eq!(chunk_sizes(&writer.finish()), vec![3, 2]);
    }

    #[test]
    fn finish_seals_a_partial_chunk_that_never_hit_a_threshold() {
        let mut writer = RotatingChunkWriter::new(RotationPolicy {
            max_encoded_size_bytes: usize::MAX,
            max_duration_ms: f64::INFINITY,
        });
        writer.push(event(0, 0.0));
        writer.push(event(1, 1.0));

        assert_eq!(chunk_sizes(&writer.finish()), vec![2]);
    }

    #[test]
    fn finishing_an_empty_writer_produces_no_chunks() {
        let writer = RotatingChunkWriter::new(RotationPolicy {
            max_encoded_size_bytes: usize::MAX,
            max_duration_ms: f64::INFINITY,
        });

        assert_eq!(writer.finish(), vec![]);
    }

    #[test]
    fn every_sealed_chunk_round_trips_through_encode_decode() {
        let mut writer = RotatingChunkWriter::new(RotationPolicy {
            max_encoded_size_bytes: usize::MAX,
            max_duration_ms: 10.0,
        });
        for (sequence, timestamp) in [0.0, 12.0, 24.0].into_iter().enumerate() {
            writer.push(event(sequence as u64, timestamp));
        }

        for chunk in writer.finish() {
            let bytes = chunk.encode().unwrap();
            assert_eq!(Chunk::decode(&bytes).unwrap(), chunk);
        }
    }
}
