package com.rnsessiontelemetry.reactnative

import android.util.Log
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import java.io.File

// Receives every event the JS library records (nativeTransfer.ts), independent of the JS
// library's own bounded in-memory buffer - the whole point of transferring to native at all is
// durability past what that sliding window keeps. Lazily opens a fresh on-device .rnst session
// (NativeSessionWriter, backed by the session-telemetry-android Rust crate via JNI) on the
// first event of each process lifetime, under this app's private files directory.
class SessionWriterModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

  private var handle: Long = 0
  private var outputDir: File? = null

  override fun getName(): String = NAME

  @Synchronized
  private fun ensureOpen(): Long {
    if (handle == 0L) {
      val dir = File(reactApplicationContext.filesDir, "rnst-sessions/${System.currentTimeMillis()}")
      dir.mkdirs()
      outputDir = dir
      handle =
          NativeSessionWriter.nativeOpen(
              dir.absolutePath,
              maxChunkBytes = DEFAULT_MAX_CHUNK_BYTES,
              maxChunkDurationMs = DEFAULT_MAX_CHUNK_DURATION_MS,
              budgetBytes = DEFAULT_BUDGET_BYTES,
          )
    }
    return handle
  }

  // Only ever transitions Warning->error once, on the push that actually crosses the budget -
  // RotatingChunkWriter (Rust) stays permanently stopped after that, so every push
  // (silently) returning OUTCOME_BUDGET_EXCEEDED afterward doesn't re-log.
  private var loggedBudgetExceeded = false

  @ReactMethod
  fun pushEvent(eventJson: String) {
    val currentHandle = ensureOpen()
    if (currentHandle == 0L) {
      return
    }
    val outcome = NativeSessionWriter.nativePushEvent(currentHandle, eventJson)
    if (outcome == NativeSessionWriter.OUTCOME_BUDGET_EXCEEDED && !loggedBudgetExceeded) {
      loggedBudgetExceeded = true
      Log.w(
          NAME,
          "On-device session storage budget ($DEFAULT_BUDGET_BYTES bytes) exceeded - " +
              "recording stopped cleanly; events captured before this point remain intact.",
      )
    }
  }

  // Exposed for verification/pull tooling to locate this session's chunk files without
  // guessing the timestamp ensureOpen() picked; null until the first event opens a session.
  @ReactMethod
  fun getOutputDirectory(promise: Promise) {
    promise.resolve(outputDir?.absolutePath)
  }

  // Seals the final in-progress chunk and frees the native writer. Not wired to SessionTelemetry
  // .stop() yet - that lifecycle wiring is separate, upcoming work (record/stop).
  @ReactMethod
  fun finish() {
    val currentHandle = handle
    if (currentHandle == 0L) {
      return
    }
    handle = 0
    NativeSessionWriter.nativeFinish(currentHandle)
  }

  companion object {
    const val NAME = "RNSessionTelemetryWriter"
    const val DEFAULT_MAX_CHUNK_BYTES = 256L * 1024
    const val DEFAULT_MAX_CHUNK_DURATION_MS = 60_000.0

    // A placeholder pending real device measurements, not a value anyone has actually
    // measured against real TV storage constraints - same caveat as every detector threshold
    // in session-telemetry-analysis. Caps total encoded size across the whole session (sealed
    // chunks and the in-progress one combined - see RotatingChunkWriter::with_budget), so a
    // multi-hour QA capture can't silently fill the device's storage.
    const val DEFAULT_BUDGET_BYTES = 200L * 1024 * 1024
  }
}
