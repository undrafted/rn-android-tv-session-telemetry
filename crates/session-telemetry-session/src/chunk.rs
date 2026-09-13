use serde::{Deserialize, Serialize};
use session_telemetry_protocol::Event;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChunkManifest {
    pub sequence_start: u64,
    pub sequence_end: u64,
    pub timestamp_start: f64,
    pub timestamp_end: f64,
    pub event_count: usize,
    /// Always 0 for now — no gap/loss detection is wired up yet. The field is real schema,
    /// just not populated with real data yet.
    pub loss_count: u32,
    /// CRC32 over the JSON encoding of `events`, computed independently of this manifest so
    /// there's no circular dependency between the checksum and the struct that stores it.
    pub checksum: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Chunk {
    pub manifest: ChunkManifest,
    pub events: Vec<Event>,
}

#[derive(Debug)]
pub enum ChunkError {
    Deserialize(serde_json::Error),
    ChecksumMismatch { expected: u32, actual: u32 },
}

impl fmt::Display for ChunkError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ChunkError::Deserialize(err) => write!(f, "failed to decode chunk: {err}"),
            ChunkError::ChecksumMismatch { expected, actual } => write!(
                f,
                "chunk checksum mismatch: manifest says {expected:#010x}, computed {actual:#010x}"
            ),
        }
    }
}

impl std::error::Error for ChunkError {}

fn checksum_of(events: &[Event]) -> Result<u32, serde_json::Error> {
    let bytes = serde_json::to_vec(events)?;
    Ok(crc32fast::hash(&bytes))
}

/// Accumulates events for one chunk. `RotatingChunkWriter` (rotation.rs) decides when to seal
/// this and start a new one — this is just the accumulate-then-seal primitive it rotates on
/// top of.
#[derive(Debug, Default)]
pub struct ChunkWriter {
    events: Vec<Event>,
}

impl ChunkWriter {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push(&mut self, event: Event) {
        self.events.push(event);
    }

    pub fn len(&self) -> usize {
        self.events.len()
    }

    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Seals the accumulated events into an independently decodable `Chunk`. Returns `None` for
    /// an empty writer — an empty chunk has no meaningful sequence/time range.
    pub fn seal(self) -> Option<Chunk> {
        if self.events.is_empty() {
            return None;
        }

        let sequence_start = self.events.iter().map(Event::sequence).min()?;
        let sequence_end = self.events.iter().map(Event::sequence).max()?;
        let timestamp_start = self
            .events
            .iter()
            .map(Event::timestamp)
            .fold(f64::INFINITY, f64::min);
        let timestamp_end = self
            .events
            .iter()
            .map(Event::timestamp)
            .fold(f64::NEG_INFINITY, f64::max);
        let checksum = checksum_of(&self.events).ok()?;

        Some(Chunk {
            manifest: ChunkManifest {
                sequence_start,
                sequence_end,
                timestamp_start,
                timestamp_end,
                event_count: self.events.len(),
                loss_count: 0,
                checksum,
            },
            events: self.events,
        })
    }
}

impl Chunk {
    pub fn encode(&self) -> Result<Vec<u8>, serde_json::Error> {
        serde_json::to_vec(self)
    }

    /// Decodes a chunk and verifies its checksum, so a truncated/corrupted chunk from an
    /// interrupted capture is rejected rather than silently trusted.
    pub fn decode(bytes: &[u8]) -> Result<Chunk, ChunkError> {
        let chunk: Chunk = serde_json::from_slice(bytes).map_err(ChunkError::Deserialize)?;
        let actual = checksum_of(&chunk.events).map_err(ChunkError::Deserialize)?;
        if actual != chunk.manifest.checksum {
            return Err(ChunkError::ChecksumMismatch {
                expected: chunk.manifest.checksum,
                actual,
            });
        }
        Ok(chunk)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::{FocusEvent, RemoteInputEvent};

    fn sample_events() -> Vec<Event> {
        vec![
            Event::RemoteInput(RemoteInputEvent {
                sequence: 0,
                timestamp: 10.0,
                key: "right".to_string(),
            }),
            Event::Focus(FocusEvent {
                sequence: 1,
                timestamp: 25.5,
                target_id: "card-1".to_string(),
                previous_target_id: None,
            }),
        ]
    }

    #[test]
    fn seal_computes_sequence_and_time_range() {
        let mut writer = ChunkWriter::new();
        for event in sample_events() {
            writer.push(event);
        }

        let chunk = writer.seal().expect("non-empty writer seals");

        assert_eq!(chunk.manifest.sequence_start, 0);
        assert_eq!(chunk.manifest.sequence_end, 1);
        assert_eq!(chunk.manifest.timestamp_start, 10.0);
        assert_eq!(chunk.manifest.timestamp_end, 25.5);
        assert_eq!(chunk.manifest.event_count, 2);
    }

    #[test]
    fn sealing_an_empty_writer_returns_none() {
        assert!(ChunkWriter::new().seal().is_none());
    }

    #[test]
    fn react_timestamps_preserve_checksum_and_round_trip() {
        for timing in ["", r#","renderStartMs":10.0,"commitTimeMs":80.0"#] {
            // Compute the stored checksum from raw JSON, independent of the new event type.
            let events = format!(
                r#"[{{"type":"react-commit","sequence":1,"timestamp":100.0,"profilerId":"App","phase":"update","actualDurationMs":5.0,"baseDurationMs":5.0{timing}}}]"#
            );
            let checksum = crc32fast::hash(events.as_bytes());

            let bytes = format!(
                r#"{{"manifest":{{"sequenceStart":1,"sequenceEnd":1,"timestampStart":100.0,"timestampEnd":100.0,"eventCount":1,"lossCount":0,"checksum":{checksum}}},"events":{events}}}"#
            );
            let chunk = Chunk::decode(bytes.as_bytes()).unwrap();
            assert_eq!(Chunk::decode(&chunk.encode().unwrap()).unwrap(), chunk);
            assert_eq!(serde_json::to_string(&chunk.events).unwrap(), events);
            if !timing.is_empty() {
                let mut tampered = chunk.clone();
                let Event::ReactCommit(event) = &mut tampered.events[0] else {
                    unreachable!()
                };
                event.commit_time_ms = Some(90.0);
                assert!(matches!(
                    Chunk::decode(&tampered.encode().unwrap()),
                    Err(ChunkError::ChecksumMismatch { .. })
                ));
            }
        }
    }

    #[test]
    fn encode_then_decode_round_trips() {
        let mut writer = ChunkWriter::new();
        for event in sample_events() {
            writer.push(event);
        }
        let chunk = writer.seal().unwrap();

        let bytes = chunk.encode().unwrap();
        let decoded = Chunk::decode(&bytes).unwrap();

        assert_eq!(decoded, chunk);
    }

    #[test]
    fn decode_rejects_a_tampered_checksum() {
        let mut writer = ChunkWriter::new();
        writer.push(sample_events().remove(0));
        let mut chunk = writer.seal().unwrap();
        chunk.manifest.checksum ^= 0xFFFF_FFFF;

        let bytes = chunk.encode().unwrap();
        let err = Chunk::decode(&bytes).unwrap_err();

        assert!(matches!(err, ChunkError::ChecksumMismatch { .. }));
    }
}
