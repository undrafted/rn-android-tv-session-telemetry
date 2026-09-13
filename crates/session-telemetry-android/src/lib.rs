//! JNI bridge exposing `session-telemetry-session`'s `RotatingChunkWriter` to the Android app,
//! so `.rnst` chunks are encoded once, in the same Rust code the host CLI decodes them with —
//! not reimplemented a second time in Kotlin, where the two could drift apart over time.
//!
//! The Kotlin side (`NativeSessionWriter`) owns an opaque `Long` handle to a boxed writer,
//! obtained from `nativeOpen` and passed back into every later call — the standard pattern for
//! exposing a stateful Rust object across a JNI boundary. Passing anything other than a handle
//! `nativeOpen` returned (0, a freed handle, a value that was never a handle at all) is
//! undefined behavior; that contract is enforced by `SessionWriterModule.kt`, not by this crate.

use std::fs;
use std::path::{Path, PathBuf};

use jni::JNIEnv;
use jni::objects::{JClass, JString};
use jni::sys::{jdouble, jint, jlong};
use session_telemetry_protocol::Event;
use session_telemetry_session::{Chunk, PushOutcome, RotatingChunkWriter, RotationPolicy};

const OUTCOME_RECORDED: jint = 0;
const OUTCOME_BUDGET_EXCEEDED: jint = 1;
const OUTCOME_DECODE_ERROR: jint = 2;
const OUTCOME_INVALID_HANDLE: jint = 3;

struct SessionWriterState {
    writer: RotatingChunkWriter,
    output_dir: PathBuf,
    next_chunk_index: u32,
}

/// A failed encode or write drops just this one chunk's evidence rather than crashing the
/// whole session — events recorded after it can still be captured, and other chunks still
/// written. `next_chunk_index` still advances even on failure, so a later successful chunk
/// never silently reuses (and overwrites) a name a failed one already claimed.
fn write_chunk(output_dir: &Path, next_chunk_index: &mut u32, chunk: &Chunk) {
    let path = output_dir.join(format!("chunk-{:05}.rnst", *next_chunk_index));
    *next_chunk_index += 1;
    if let Ok(bytes) = chunk.encode() {
        let _ = fs::write(path, bytes);
    }
}

/// Opens a new on-device session writer rooted at `output_dir`, which must already exist.
/// `budget_bytes <= 0` means no total-storage budget. Returns an opaque handle for
/// `nativePushEvent`/`nativeFinish`, or 0 if `output_dir` isn't valid UTF-8.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rnsessiontelemetry_reactnative_NativeSessionWriter_nativeOpen(
    mut env: JNIEnv,
    _class: JClass,
    output_dir: JString,
    max_chunk_bytes: jlong,
    max_chunk_duration_ms: jdouble,
    budget_bytes: jlong,
) -> jlong {
    let output_dir: String = match env.get_string(&output_dir) {
        Ok(value) => value.into(),
        Err(_) => return 0,
    };

    let policy = RotationPolicy {
        max_encoded_size_bytes: max_chunk_bytes.max(0) as usize,
        max_duration_ms: max_chunk_duration_ms,
    };
    let mut writer = RotatingChunkWriter::new(policy);
    if budget_bytes > 0 {
        writer = writer.with_budget(budget_bytes as usize);
    }

    let state = Box::new(SessionWriterState {
        writer,
        output_dir: PathBuf::from(output_dir),
        next_chunk_index: 0,
    });
    Box::into_raw(state) as jlong
}

/// Decodes `event_json` (the same wire format `session-telemetry-protocol::Event` always
/// round-trips through) and pushes it into the writer at `handle`, writing a chunk file
/// immediately if this push triggered a rotation — the writer's own bounded JS buffer already
/// covers in-memory recency; this on-device file is what survives past the JS process dying.
/// Returns an OUTCOME_* code.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rnsessiontelemetry_reactnative_NativeSessionWriter_nativePushEvent(
    mut env: JNIEnv,
    _class: JClass,
    handle: jlong,
    event_json: JString,
) -> jint {
    if handle == 0 {
        return OUTCOME_INVALID_HANDLE;
    }
    // SAFETY: `handle` is a pointer `nativeOpen` returned via `Box::into_raw` and not yet
    // passed to `nativeFinish` — guaranteed by SessionWriterModule.kt, not by this function.
    let state = unsafe { &mut *(handle as *mut SessionWriterState) };

    let event_json: String = match env.get_string(&event_json) {
        Ok(value) => value.into(),
        Err(_) => return OUTCOME_DECODE_ERROR,
    };
    let event: Event = match serde_json::from_str(&event_json) {
        Ok(event) => event,
        Err(_) => return OUTCOME_DECODE_ERROR,
    };

    let (outcome, sealed_chunk) = state.writer.push_and_take_sealed(event);
    if let Some(chunk) = &sealed_chunk {
        write_chunk(&state.output_dir, &mut state.next_chunk_index, chunk);
    }

    match outcome {
        PushOutcome::Recorded => OUTCOME_RECORDED,
        PushOutcome::BudgetExceeded => OUTCOME_BUDGET_EXCEEDED,
    }
}

/// Seals whatever's left accumulated, writes it out, and frees the writer at `handle` — it
/// must not be used again after this call. Returns the total number of chunk files this writer
/// produced across its whole lifetime, or -1 for an already-null handle.
#[unsafe(no_mangle)]
pub extern "system" fn Java_com_rnsessiontelemetry_reactnative_NativeSessionWriter_nativeFinish(
    _env: JNIEnv,
    _class: JClass,
    handle: jlong,
) -> jint {
    if handle == 0 {
        return -1;
    }
    // SAFETY: same contract as nativePushEvent, plus this is the one call allowed to actually
    // reclaim the box - the caller must not pass this handle to any function again afterward.
    let state = unsafe { Box::from_raw(handle as *mut SessionWriterState) };
    let SessionWriterState {
        writer,
        output_dir,
        mut next_chunk_index,
    } = *state;

    for chunk in &writer.finish() {
        write_chunk(&output_dir, &mut next_chunk_index, chunk);
    }

    next_chunk_index as jint
}

#[cfg(test)]
mod tests {
    use super::*;
    use session_telemetry_protocol::RemoteInputEvent;
    use session_telemetry_session::ChunkWriter;

    #[test]
    fn write_chunk_names_files_by_an_incrementing_counter_and_leaves_valid_rnst_bytes() {
        let dir = std::env::temp_dir().join(format!("rnst-test-{}", std::process::id()));
        fs::create_dir_all(&dir).unwrap();
        let mut next_chunk_index = 0;

        let mut writer = ChunkWriter::new();
        writer.push(Event::RemoteInput(RemoteInputEvent {
            sequence: 0,
            timestamp: 0.0,
            key: "right".to_string(),
        }));
        let chunk = writer.seal().unwrap();

        write_chunk(&dir, &mut next_chunk_index, &chunk);
        write_chunk(&dir, &mut next_chunk_index, &chunk);

        assert_eq!(next_chunk_index, 2);
        let first_bytes = fs::read(dir.join("chunk-00000.rnst")).unwrap();
        assert_eq!(Chunk::decode(&first_bytes).unwrap(), chunk);
        assert!(dir.join("chunk-00001.rnst").exists());

        fs::remove_dir_all(&dir).ok();
    }
}
