package com.rnsessiontelemetry.reactnative

import android.os.Debug
import android.os.Handler
import android.os.HandlerThread
import android.os.Process
import android.os.SystemClock
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.modules.core.DeviceEventManagerModule

// A HandlerThread-based periodic CPU/memory sampler - the same pattern FrameTimingModule uses,
// running off the UI thread so sampling itself never contends with layout/draw. Unlike every
// other native module in this library, this one is never started automatically: the JS side
// (SessionTelemetry.startResourceSampling()) only calls start() when the application explicitly
// opens a sampling window, because of the overhead a resource sampler carries (see
// ResourceSamplingOptions' own doc comment in index.ts).
//
// CPU: Process.getElapsedCpuTime() is the process-wide CPU time (all threads, kernel-scheduler
// measured) since the process started, so a utilization percentage needs two readings - this
// tracks the previous sample's CPU time and wall time and reports the delta ratio, not a
// cumulative or instantaneous OS-level percentage. Normalized to one core (see the doc comment
// on ResourceSampleEvent in event.rs): it can read above 100 on a multi-core device actively
// using more than one thread, and this is never method-level attribution.
//
// Memory: Debug.getNativeHeapAllocatedSize() and the JVM Runtime's used-heap figure are both
// cheap, in-process reads - deliberately not ActivityManager.getProcessMemoryInfo, which is
// documented as a relatively expensive cross-process query (a binder call to system_server) and
// would itself add overhead to the very measurement meant to stay lightweight.
class ResourceSamplingModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

  private var handlerThread: HandlerThread? = null
  private var handler: Handler? = null
  private var lastCpuTimeMs: Long = 0
  private var lastWallTimeMs: Long = 0

  override fun getName(): String = NAME

  @ReactMethod
  fun start(intervalMs: Double) {
    stopInternal()
    val thread = HandlerThread("RNSessionTelemetryResourceSampling").also { it.start() }
    handlerThread = thread
    val threadHandler = Handler(thread.looper)
    handler = threadHandler
    lastCpuTimeMs = Process.getElapsedCpuTime()
    lastWallTimeMs = SystemClock.elapsedRealtime()
    scheduleNext(threadHandler, intervalMs.toLong().coerceAtLeast(16))
  }

  @ReactMethod
  fun stop() {
    stopInternal()
  }

  private fun stopInternal() {
    handler?.removeCallbacksAndMessages(null)
    handler = null
    handlerThread?.quitSafely()
    handlerThread = null
  }

  private fun scheduleNext(threadHandler: Handler, intervalMs: Long) {
    threadHandler.postDelayed(
        {
          sampleAndEmit()
          scheduleNext(threadHandler, intervalMs)
        },
        intervalMs,
    )
  }

  private fun sampleAndEmit() {
    val cpuTimeMs = Process.getElapsedCpuTime()
    val wallTimeMs = SystemClock.elapsedRealtime()
    val wallDeltaMs = wallTimeMs - lastWallTimeMs
    val cpuUtilizationPercent =
        if (wallDeltaMs > 0) {
          100.0 * (cpuTimeMs - lastCpuTimeMs) / wallDeltaMs
        } else {
          0.0
        }
    lastCpuTimeMs = cpuTimeMs
    lastWallTimeMs = wallTimeMs

    val nativeHeapKb = Debug.getNativeHeapAllocatedSize() / 1024
    val runtime = Runtime.getRuntime()
    val javaHeapKb = (runtime.totalMemory() - runtime.freeMemory()) / 1024

    val params = Arguments.createMap()
    params.putDouble("cpuUtilizationPercent", cpuUtilizationPercent)
    params.putDouble("nativeHeapKb", nativeHeapKb.toDouble())
    params.putDouble("javaHeapKb", javaHeapKb.toDouble())
    reactApplicationContext
        .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
        .emit(EVENT_SAMPLE, params)
  }

  // NativeEventEmitter (resourceSampling.ts) requires these on any native module it wraps, even
  // though events are emitted directly via RCTDeviceEventEmitter above rather than through this
  // module's own add/remove-listener bookkeeping - without them it logs a warning on every use
  // (same as FrameTimingModule/GlobalFocusModule).
  @ReactMethod
  fun addListener(eventName: String) {}

  @ReactMethod
  fun removeListeners(count: Int) {}

  companion object {
    const val NAME = "RNSessionTelemetryResourceSampling"
    const val EVENT_SAMPLE = "RNSessionTelemetryResourceSampling.sample"
  }
}
