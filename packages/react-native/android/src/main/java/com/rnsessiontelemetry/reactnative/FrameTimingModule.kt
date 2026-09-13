package com.rnsessiontelemetry.reactnative

import android.os.Handler
import android.os.HandlerThread
import android.view.FrameMetrics
import android.view.Window
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.modules.core.DeviceEventManagerModule

// Uses Window.OnFrameMetricsAvailableListener (FrameMetrics, public API since 24 - this
// project's minSdkVersion) rather than Choreographer, so "was this frame late" comes from
// Android's own per-frame duration breakdown instead of a hand-rolled vsync-gap heuristic. Only
// frames whose total duration exceeds thresholdMs are forwarded to JS - at 60Hz every frame
// would otherwise cross the bridge, which the bounded JS buffer (see index.ts) is specifically
// designed to avoid growing unnecessarily fast from.
class FrameTimingModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext), Window.OnFrameMetricsAvailableListener {

  private var handlerThread: HandlerThread? = null
  private var thresholdNs: Long = 0

  override fun getName(): String = NAME

  @ReactMethod
  fun start(thresholdMs: Double) {
    thresholdNs = (thresholdMs * NANOS_PER_MILLI).toLong()
    val window = reactApplicationContext.currentActivity?.window ?: return
    val thread = HandlerThread("RNSessionTelemetryFrameTiming").also { it.start() }
    handlerThread = thread
    window.addOnFrameMetricsAvailableListener(this, Handler(thread.looper))
  }

  @ReactMethod
  fun stop() {
    reactApplicationContext.currentActivity?.window?.removeOnFrameMetricsAvailableListener(this)
    handlerThread?.quitSafely()
    handlerThread = null
  }

  // Called on the HandlerThread passed to addOnFrameMetricsAvailableListener, not the UI thread.
  override fun onFrameMetricsAvailable(
      window: Window,
      frameMetrics: FrameMetrics,
      dropCountSinceLastInvocation: Int,
  ) {
    val totalDurationNs = frameMetrics.getMetric(FrameMetrics.TOTAL_DURATION)
    if (totalDurationNs <= thresholdNs) {
      return
    }
    val params = Arguments.createMap()
    params.putDouble("durationMs", totalDurationNs / NANOS_PER_MILLI)
    reactApplicationContext
        .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
        .emit(EVENT_NAME, params)
  }

  companion object {
    const val NAME = "RNSessionTelemetryFrameTiming"
    const val EVENT_NAME = "RNSessionTelemetryFrameTiming.delayedFrame"
    const val NANOS_PER_MILLI = 1_000_000.0
  }
}
