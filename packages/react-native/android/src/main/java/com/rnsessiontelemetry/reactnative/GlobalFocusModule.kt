package com.rnsessiontelemetry.reactnative

import android.view.View
import android.view.ViewTreeObserver
import com.facebook.react.R
import com.facebook.react.bridge.Arguments
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import com.facebook.react.modules.core.DeviceEventManagerModule

// Replaces per-component focus tracking (FocusableView, now removed) with one app-wide listener:
// ViewTreeObserver.OnGlobalFocusChangeListener fires for every focus change across the whole
// view hierarchy, not just views a custom wrapper component opted in. A view is identified by
// its `nativeID` prop - a standard React Native prop (not telemetry-specific; many apps already
// set it for test automation), bridged by RN itself onto the native View as
// R.id.view_tag_native_id (BaseViewManager.setTag). A view with no nativeID is silently skipped:
// that absence *is* the opt-in mechanism, same spirit as every other explicit-marker signal in
// this library - there's no per-component code to write, only an existing prop to set.
class GlobalFocusModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext), ViewTreeObserver.OnGlobalFocusChangeListener {

  override fun getName(): String = NAME

  @ReactMethod
  fun start() {
    val decorView = reactApplicationContext.currentActivity?.window?.decorView ?: return
    decorView.viewTreeObserver.addOnGlobalFocusChangeListener(this)
  }

  @ReactMethod
  fun stop() {
    val decorView = reactApplicationContext.currentActivity?.window?.decorView ?: return
    decorView.viewTreeObserver.removeOnGlobalFocusChangeListener(this)
  }

  // NativeEventEmitter (globalFocus.ts) requires these on any native module it wraps, even
  // though events are emitted directly via RCTDeviceEventEmitter above rather than through this
  // module's own add/remove-listener bookkeeping - without them it logs a warning on every use
  // (same as FrameTimingModule).
  @ReactMethod
  fun addListener(eventName: String) {}

  @ReactMethod
  fun removeListeners(count: Int) {}

  override fun onGlobalFocusChanged(oldFocus: View?, newFocus: View?) {
    val targetId = newFocus?.getTag(R.id.view_tag_native_id) as? String ?: return
    emit(EVENT_FOCUS, targetId)
    // Posted, not emitted immediately: runs after the current layout/draw pass settles on this
    // view's own UI-thread message queue - a best-effort confirmation the focus change's visual
    // result was scheduled to paint, tied to Android's own native pipeline rather than a JS-side
    // requestAnimationFrame guess. Still not a hard guarantee of the exact composited frame.
    newFocus.post { emit(EVENT_VISIBLE_UPDATE, targetId) }
  }

  private fun emit(eventName: String, targetId: String) {
    val params = Arguments.createMap()
    params.putString("targetId", targetId)
    reactApplicationContext
        .getJSModule(DeviceEventManagerModule.RCTDeviceEventEmitter::class.java)
        .emit(eventName, params)
  }

  companion object {
    const val NAME = "RNSessionTelemetryGlobalFocus"
    const val EVENT_FOCUS = "RNSessionTelemetryGlobalFocus.focusChanged"
    const val EVENT_VISIBLE_UPDATE = "RNSessionTelemetryGlobalFocus.visibleUpdate"
  }
}
