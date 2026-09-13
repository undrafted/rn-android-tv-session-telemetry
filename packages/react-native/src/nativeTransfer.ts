import { NativeModules } from 'react-native';
import type { SessionTelemetryEvent } from './events.js';

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
