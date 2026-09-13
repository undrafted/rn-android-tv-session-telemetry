package com.rnsessiontelemetry.reactnative

import com.facebook.react.bridge.Promise
import com.facebook.react.bridge.ReactApplicationContext
import com.facebook.react.bridge.ReactContextBaseJavaModule
import com.facebook.react.bridge.ReactMethod
import java.util.Collections

// Receives every event the JS library records (nativeTransfer.ts), independent of the JS
// library's own bounded in-memory buffer - the whole point of transferring to native at all is
// durability past what that sliding window keeps. Deliberately just a JSON-string sink for now:
// it doesn't decode events, only session-telemetry-protocol (Rust) needs to understand their
// shape. The on-device session writer (a separate piece of work) hands these off to that Rust
// code via JNI to actually encode them into .rnst chunks - this module only needs to hold them
// until that handoff happens.
class SessionWriterModule(reactContext: ReactApplicationContext) :
    ReactContextBaseJavaModule(reactContext) {

  // CopyOnWriteArrayList would be a poor choice for what could be a fast, ordered append
  // stream (O(n) per write); a synchronized ArrayList keeps appends O(1) amortized while still
  // being safe against pushEvent() and getBufferedEventCount() overlapping across threads.
  private val bufferedEventsJson = Collections.synchronizedList(mutableListOf<String>())

  override fun getName(): String = NAME

  @ReactMethod
  fun pushEvent(eventJson: String) {
    bufferedEventsJson.add(eventJson)
  }

  @ReactMethod
  fun getBufferedEventCount(promise: Promise) {
    promise.resolve(bufferedEventsJson.size)
  }

  companion object {
    const val NAME = "RNSessionTelemetryWriter"
  }
}
