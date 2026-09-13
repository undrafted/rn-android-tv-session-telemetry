package com.rnsessiontelemetry.reactnative

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
              budgetBytes = NO_BUDGET,
          )
    }
    return handle
  }

  @ReactMethod
  fun pushEvent(eventJson: String) {
    val currentHandle = ensureOpen()
    if (currentHandle == 0L) {
      return
    }
    NativeSessionWriter.nativePushEvent(currentHandle, eventJson)
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

    // No total-storage budget yet - real device-storage budgeting is separate, upcoming work.
    const val NO_BUDGET = 0L
  }
}
