package com.rnsessiontelemetry.reactnative

import android.content.BroadcastReceiver
import android.content.Context
import android.content.Intent
import android.content.IntentFilter
import android.os.Build
import android.util.Log
import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.modules.core.DeviceEventManagerModule
import java.io.File

// Receives every event the JS library records (nativeTransfer.ts), independent of the JS
// library's own bounded in-memory buffer - the whole point of transferring to native at all is
// durability past what that sliding window keeps. Writes to a fresh on-device .rnst session
// (NativeSessionWriter, backed by the session-telemetry-android Rust crate via JNI) under this
// app's private files directory, only while a session is actually open.
//
// No session opens implicitly: pushEvent() with no session open is a no-op (mirrors the JS
// library's own `installed` gate in index.ts), and nothing here opens one lazily. A session
// starts only via an explicit start(), which is bidirectional - both the app itself
// (SessionTelemetry.install() in index.ts, JS-callable via the @ReactMethod below) and
// `session-telemetry record`/`stop` (workstation side, via `adb shell am broadcast` - see
// ACTION_START_SESSION/ACTION_STOP_SESSION) can start or end one.
class SessionWriterModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

  private var handle: Long = 0
  private var outputDir: File? = null

  // Only ever transitions false->true once, on the push that actually crosses the budget -
  // RotatingChunkWriter (Rust) stays permanently stopped after that, so every push (silently)
  // returning OUTCOME_BUDGET_EXCEEDED afterward doesn't re-log. Reset on each fresh session.
  private var loggedBudgetExceeded = false

  // Registered dynamically (not in the manifest) rather than as a manifest-declared receiver,
  // so it works with zero app-side wiring beyond this library being linked, and isn't subject
  // to Android's background-execution limits on manifest receivers - this one only needs to
  // work while the app process (and therefore this module) is alive, which is exactly when a
  // session could be open anyway.
  private val sessionControlReceiver =
      object : BroadcastReceiver() {
        override fun onReceive(context: Context, intent: Intent) {
          when (intent.action) {
            ACTION_START_SESSION -> startFreshSession()
            ACTION_STOP_SESSION -> finishSession()
          }
        }
      }

  override fun initialize() {
    super.initialize()
    val filter =
        IntentFilter().apply {
          addAction(ACTION_START_SESSION)
          addAction(ACTION_STOP_SESSION)
        }
    // RECEIVER_EXPORTED (API 33+) is required, not RECEIVER_NOT_EXPORTED: `adb shell am
    // broadcast` is sent from the shell's own process/UID, a different app from ours, so this
    // receiver must be reachable cross-app. Below API 33 there's no such flag to pass at all.
    if (Build.VERSION.SDK_INT >= Build.VERSION_CODES.TIRAMISU) {
      reactApplicationContext.registerReceiver(
          sessionControlReceiver,
          filter,
          Context.RECEIVER_EXPORTED,
      )
    } else {
      @Suppress("UnspecifiedRegisterReceiverFlag")
      reactApplicationContext.registerReceiver(sessionControlReceiver, filter)
    }
  }

  override fun invalidate() {
    super.invalidate()
    // Only ever throws if already unregistered (or never registered) - neither is a real error
    // here, just the module tearing down more than once.
    runCatching { reactApplicationContext.unregisterReceiver(sessionControlReceiver) }
  }

  override fun getName(): String = NAME

  @Synchronized
  private fun currentHandle(): Long = handle

  // Shared by both trigger paths (JS-callable start() and the ACTION_START_SESSION broadcast):
  // any currently-open writer is sealed first, then a new one begins, so calling this always
  // yields a clean session boundary regardless of what was already open.
  @Synchronized
  private fun startFreshSession() {
    finishSessionLocked()
    openNewSessionLocked()
  }

  @Synchronized
  private fun finishSession() {
    finishSessionLocked()
  }

  // Callers must already hold this module's monitor - only called from the @Synchronized
  // methods above, via Kotlin/Java's reentrant `synchronized`.
  private fun openNewSessionLocked() {
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
    loggedBudgetExceeded = false
    // Tells JS a fresh native session just opened (nativeTransfer.ts's onNativeSessionOpened),
    // so it can push an immediate clock-sync sample into *this* session even when this open was
    // triggered by ACTION_START_SESSION (`session-telemetry record`), not by the app's own
    // SessionTelemetry.install() - the two are independent levers by design (see this class's
    // own doc comment), so JS has no other way to know a new native session boundary just
    // happened. Without this, a native session that starts long after JS's own install() (and
    // therefore long after its own one-time baseline sample) could go its entire lifetime with
    // zero clock-sync coverage if it's shorter than the periodic sampling interval - confirmed
    // as a real gap against a live device, not a hypothetical.
    reactApplicationContext
        .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
        .emit(EVENT_SESSION_OPENED, null)
  }

  private fun finishSessionLocked() {
    val currentHandle = handle
    if (currentHandle == 0L) {
      return
    }
    handle = 0
    NativeSessionWriter.nativeFinish(currentHandle)
  }

  @ReactMethod
  fun pushEvent(eventJson: String) {
    val handle = currentHandle()
    if (handle == 0L) {
      return
    }
    val outcome = NativeSessionWriter.nativePushEvent(handle, eventJson)
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
  // guessing the timestamp openNewSessionLocked() picked; null until a session has opened.
  @ReactMethod
  fun getOutputDirectory(promise: Promise) {
    promise.resolve(outputDir?.absolutePath)
  }

  // The JS-callable equivalent of the ACTION_START_SESSION broadcast - called from
  // SessionTelemetry.install() (index.ts) so the app's own enable function actually starts
  // durable on-device recording, not just the JS-side buffer/subscriptions.
  @ReactMethod
  fun start() {
    startFreshSession()
  }

  // Seals the final in-progress chunk and frees the native writer - the JS-callable equivalent
  // of the ACTION_STOP_SESSION broadcast, called from SessionTelemetry.stop() (index.ts) so the
  // app's own disable function actually seals recording, not just stopping JS-side capture.
  @ReactMethod
  fun finish() {
    finishSession()
  }

  // NativeEventEmitter (nativeTransfer.ts's onNativeSessionOpened) requires these on any native
  // module it wraps, even though EVENT_SESSION_OPENED is emitted directly via
  // RCTDeviceEventEmitter above rather than through this module's own add/remove-listener
  // bookkeeping - without them it logs a warning on every use (same as FrameTimingModule).
  @ReactMethod
  fun addListener(eventName: String) {}

  @ReactMethod
  fun removeListeners(count: Int) {}

  companion object {
    const val NAME = "RNSessionTelemetryWriter"
    const val EVENT_SESSION_OPENED = "RNSessionTelemetryWriter.sessionOpened"
    const val DEFAULT_MAX_CHUNK_BYTES = 256L * 1024
    const val DEFAULT_MAX_CHUNK_DURATION_MS = 60_000.0

    // A placeholder pending real device measurements, not a value anyone has actually
    // measured against real TV storage constraints - same caveat as every detector threshold
    // in session-telemetry-analysis. Caps total encoded size across the whole session (sealed
    // chunks and the in-progress one combined - see RotatingChunkWriter::with_budget), so a
    // multi-hour QA capture can't silently fill the device's storage.
    const val DEFAULT_BUDGET_BYTES = 200L * 1024 * 1024

    // Mirrored exactly in session-telemetry-cli/src/main.rs's send_broadcast calls - there's no
    // shared-constant mechanism across Kotlin and Rust, so keep both sides in sync by hand if
    // either changes.
    const val ACTION_START_SESSION = "com.rnsessiontelemetry.reactnative.action.START_SESSION"
    const val ACTION_STOP_SESSION = "com.rnsessiontelemetry.reactnative.action.STOP_SESSION"
  }
}
