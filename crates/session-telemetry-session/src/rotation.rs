use crate::chunk::{Chunk, ChunkWriter};
use session_telemetry_protocol::Event;

/// When to rotate to a new chunk: after a configured duration or encoded size. Either threshold
/// being crossed triggers a rotation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RotationPolicy {
    pub max_encoded_size_bytes: usize,
    pub max_duration_ms: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PushOutcome {
    Recorded,
    /// The total-storage budget is exhausted; this event was rejected and the writer will
    /// reject every subsequent one too. Plan.md section 5: "When the configured disk budget is
    /// reached, V1 stops recording cleanly and seals the session. It does not silently
    /// overwrite earlier evidence." Everything accepted before this point is untouched and
    /// still retrievable via `finish()`.
    BudgetExceeded,
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
    budget_bytes: Option<usize>,
    total_encoded_size_bytes: usize,
    stopped: bool,
}

impl RotatingChunkWriter {
    pub fn new(policy: RotationPolicy) -> Self {
        Self {
            policy,
            current: ChunkWriter::new(),
            current_encoded_size_bytes: 0,
            current_start_timestamp: None,
            sealed: Vec::new(),
            budget_bytes: None,
            total_encoded_size_bytes: 0,
            stopped: false,
        }
    }

    /// Caps the total encoded size across every chunk this writer produces (sealed and
    /// in-progress combined).
    pub fn with_budget(mut self, max_total_encoded_size_bytes: usize) -> Self {
        self.budget_bytes = Some(max_total_encoded_size_bytes);
        self
    }

    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    pub fn total_encoded_size_bytes(&self) -> usize {
        self.total_encoded_size_bytes
    }

    pub fn push(&mut self, event: Event) -> PushOutcome {
        if self.stopped {
            return PushOutcome::BudgetExceeded;
        }

        // Encoded size of just this one event — an incrementally maintained running total,
        // not re-serializing the whole accumulated chunk on every push.
        let event_size_bytes = serde_json::to_vec(&event).map_or(0, |bytes| bytes.len());

        if let Some(budget) = self.budget_bytes
            && self.total_encoded_size_bytes + event_size_bytes > budget
        {
            self.stopped = true;
            return PushOutcome::BudgetExceeded;
        }

        let timestamp = event.timestamp();
        let start_timestamp = *self.current_start_timestamp.get_or_insert(timestamp);

        self.current.push(event);
        self.current_encoded_size_bytes += event_size_bytes;
        self.total_encoded_size_bytes += event_size_bytes;

        let duration_ms = timestamp - start_timestamp;
        let exceeds_size = self.current_encoded_size_bytes >= self.policy.max_encoded_size_bytes;
        let exceeds_duration = duration_ms >= self.policy.max_duration_ms;

        if exceeds_size || exceeds_duration {
            self.rotate();
        }

        PushOutcome::Recorded
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

    fn writer_without_rotation() -> RotatingChunkWriter {
        RotatingChunkWriter::new(RotationPolicy {
            max_encoded_size_bytes: usize::MAX,
            max_duration_ms: f64::INFINITY,
        })
    }

    #[test]
    fn without_a_budget_every_push_is_recorded() {
        let mut writer = writer_without_rotation();

        for sequence in 0..5 {
            assert_eq!(
                writer.push(event(sequence, sequence as f64)),
                PushOutcome::Recorded
            );
        }
    }

    #[test]
    fn stops_recording_once_the_budget_is_exhausted() {
        let one_event_size = serde_json::to_vec(&event(0, 0.0)).unwrap().len();
        let mut writer = writer_without_rotation().with_budget(one_event_size * 3);

        for sequence in 0..3 {
            assert_eq!(
                writer.push(event(sequence, sequence as f64)),
                PushOutcome::Recorded
            );
        }
        assert_eq!(writer.push(event(3, 3.0)), PushOutcome::BudgetExceeded);
        assert!(writer.is_stopped());
        assert_eq!(writer.total_encoded_size_bytes(), one_event_size * 3);
    }

    #[test]
    fn stays_stopped_and_keeps_earlier_evidence_once_the_budget_is_hit() {
        let one_event_size = serde_json::to_vec(&event(0, 0.0)).unwrap().len();
        let mut writer = writer_without_rotation().with_budget(one_event_size);

        assert_eq!(writer.push(event(0, 0.0)), PushOutcome::Recorded);
        assert_eq!(writer.push(event(1, 1.0)), PushOutcome::BudgetExceeded);
        // Doesn't un-stick even if a later, smaller event would technically still fit.
        assert_eq!(writer.push(event(2, 2.0)), PushOutcome::BudgetExceeded);

        assert_eq!(chunk_sizes(&writer.finish()), vec![1]);
    }
}
