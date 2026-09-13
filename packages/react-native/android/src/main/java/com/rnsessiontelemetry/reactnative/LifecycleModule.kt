package com.rnsessiontelemetry.reactnative

import android.app.Activity
import android.app.Application
import android.os.Bundle
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.modules.core.DeviceEventManagerModule

// Tracks the whole app's foreground/background state via Application.ActivityLifecycleCallbacks'
// started/stopped counts, not a single Activity's own state - a startedCount that goes from 0 to
// 1 means the app just came to the foreground (whichever Activity did it); a count that drops
// from 1 to 0 means every Activity has stopped, i.e. the app is now backgrounded. This is the
// same technique AndroidX's ProcessLifecycleOwner uses internally, done directly here rather than
// pulling in that dependency for one counter. Counting only (not a boolean) matters if a host app
// ever has more than one Activity: a second Activity starting while the first is still visible
// (e.g. a picture-in-picture transition) must not be read as a fresh foreground entry.
class LifecycleModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext), Application.ActivityLifecycleCallbacks {

  private var startedCount = 0

  override fun getName(): String = NAME

  @ReactMethod
  fun start() {
    val application = reactApplicationContext.applicationContext as? Application ?: return
    application.registerActivityLifecycleCallbacks(this)
  }

  @ReactMethod
  fun stop() {
    val application = reactApplicationContext.applicationContext as? Application ?: return
    application.unregisterActivityLifecycleCallbacks(this)
  }

  override fun onActivityStarted(activity: Activity) {
    startedCount += 1
    if (startedCount == 1) {
      emit(STATE_FOREGROUND)
    }
  }

  override fun onActivityStopped(activity: Activity) {
    startedCount = (startedCount - 1).coerceAtLeast(0)
    if (startedCount == 0) {
      emit(STATE_BACKGROUND)
    }
  }

  override fun onActivityCreated(activity: Activity, savedInstanceState: Bundle?) {}

  override fun onActivityResumed(activity: Activity) {}

  override fun onActivityPaused(activity: Activity) {}

  override fun onActivitySaveInstanceState(activity: Activity, outState: Bundle) {}

  override fun onActivityDestroyed(activity: Activity) {}

  private fun emit(state: String) {
    val params = Arguments.createMap()
    params.putString("state", state)
    reactApplicationContext
        .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
        .emit(EVENT_TRANSITION, params)
  }

  // NativeEventEmitter (lifecycle.ts) requires these on any native module it wraps, even though
  // events are emitted directly via RCTDeviceEventEmitter above rather than through this module's
  // own add/remove-listener bookkeeping - without them it logs a warning on every use (same as
  // every other module in this package).
  @ReactMethod
  fun addListener(eventName: String) {}

  @ReactMethod
  fun removeListeners(count: Int) {}

  companion object {
    const val NAME = "RNSessionTelemetryLifecycle"
    const val EVENT_TRANSITION = "RNSessionTelemetryLifecycle.transition"
    const val STATE_FOREGROUND = "foreground"
    const val STATE_BACKGROUND = "background"
  }
}
