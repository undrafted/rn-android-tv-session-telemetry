//! `.rnst` session format: chunk encoding, sealing, and validation.

mod chunk;
mod rotation;

pub use chunk::{Chunk, ChunkError, ChunkManifest, ChunkWriter};
pub use rotation::{PushOutcome, RotatingChunkWriter, RotationPolicy};
