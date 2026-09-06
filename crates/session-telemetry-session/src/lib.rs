//! `.rnst` session format: chunk encoding, sealing, and validation.

mod chunk;

pub use chunk::{Chunk, ChunkError, ChunkManifest, ChunkWriter, SCHEMA_VERSION};
