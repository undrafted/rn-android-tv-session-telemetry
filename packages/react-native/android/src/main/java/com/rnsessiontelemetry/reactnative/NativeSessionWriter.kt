package com.rnsessiontelemetry.reactnative

// Thin JNI declarations matching session-telemetry-android's exported symbols (crates/
// session-telemetry-android/src/lib.rs) - reuses session-telemetry-session's chunk/checksum
// code directly rather than reimplementing the .rnst format a second time in Kotlin, so there's
// exactly one implementation of that format for the host CLI's decoder to stay compatible with.
object NativeSessionWriter {
  const val OUTCOME_RECORDED = 0
  const val OUTCOME_BUDGET_EXCEEDED = 1
  const val OUTCOME_DECODE_ERROR = 2
  const val OUTCOME_INVALID_HANDLE = 3

  init {
    System.loadLibrary("session_telemetry_android")
  }

  // Returns an opaque handle for nativePushEvent/nativeFinish, or 0 if outputDir wasn't valid
  // UTF-8. budgetBytes <= 0 means no total-storage budget.
  external fun nativeOpen(
      outputDir: String,
      maxChunkBytes: Long,
      maxChunkDurationMs: Double,
      budgetBytes: Long,
  ): Long

  external fun nativePushEvent(handle: Long, eventJson: String): Int

  // Must not be called more than once, and handle must not be used again afterward - the Rust
  // side frees it here.
  external fun nativeFinish(handle: Long): Int
}
