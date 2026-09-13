import { NativeEventEmitter, NativeModules } from 'react-native';
import type { SessionTelemetryEvent } from './events.js';

const SESSION_OPENED_EVENT = 'RNSessionTelemetryWriter.sessionOpened';

interface NativeSessionWriter {
  pushEvent(eventJson: string): void;
  start(): void;
  finish(): void;
}

function nativeWriter(): NativeSessionWriter | undefined {
  return NativeModules.RNSessionTelemetryWriter as NativeSessionWriter | undefined;
}

// Forwards every recorded event to the native RNSessionTelemetryWriter module (android/), which
// holds them for the on-device session writer to encode into .rnst chunks. JSON-encoding here
// (rather than passing a native map) keeps this bridge call trivial and lets the native side
// stay a dumb byte-string sink - it doesn't need to know this library's event shapes at all,
// only the host's existing JSON decoder (session-telemetry-protocol) does. No-ops when the
// native module isn't linked (e.g. iOS, or a JS-only test host), the same graceful-absence
// behavior as every other optional signal in this library.
export function transferEventToNative(event: SessionTelemetryEvent): void {
  nativeWriter()?.pushEvent(JSON.stringify(event));
}

// Called from SessionTelemetry.install() so the library's own enable function actually starts
// durable on-device recording, not just the JS-side buffer/subscriptions - bidirectional with
// `session-telemetry record` (workstation side, via an ADB broadcast the same native module
// also listens for). No session opens implicitly on either side; something must call this.
export function startNativeSession(): void {
  nativeWriter()?.start();
}

// Called from SessionTelemetry.stop() so the library's own disable function actually seals the
// on-device session (the last in-progress chunk included), not just stopping JS-side capture -
// bidirectional counterpart to `session-telemetry stop`.
export function finishNativeSession(): void {
  nativeWriter()?.finish();
}

// Subscribes to the native module's notification that a fresh native session just opened -
// fired for *either* trigger (this app's own startNativeSession() above, or a
// `session-telemetry record` ADB broadcast the native module also listens for), since JS has no
// other way to observe a broadcast-triggered native session boundary. index.ts uses this to
// force an immediate clock-sync sample into that session rather than waiting on its own
// periodic interval, which a short session could otherwise entirely outlast. Returns a no-op
// unsubscribe when the native module isn't linked, the same graceful-absence behavior as every
// other optional signal in this library.
export function onNativeSessionOpened(callback: () => void): () => void {
  const nativeModule = NativeModules.RNSessionTelemetryWriter as object | undefined;
  if (!nativeModule) {
    return () => {};
  }
  const emitter = new NativeEventEmitter(nativeModule as never);
  const subscription = emitter.addListener(SESSION_OPENED_EVENT, callback);
  return () => subscription.remove();
}
