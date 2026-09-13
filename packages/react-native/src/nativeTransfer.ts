import { NativeModules } from 'react-native';
import type { SessionTelemetryEvent } from './events.js';

// Forwards every recorded event to the native RNSessionTelemetryWriter module (android/), which
// holds them for the on-device session writer to encode into .rnst chunks. JSON-encoding here
// (rather than passing a native map) keeps this bridge call trivial and lets the native side
// stay a dumb byte-string sink - it doesn't need to know this library's event shapes at all,
// only the host's existing JSON decoder (session-telemetry-protocol) does. No-ops when the
// native module isn't linked (e.g. iOS, or a JS-only test host), the same graceful-absence
// behavior as every other optional signal in this library.
export function transferEventToNative(event: SessionTelemetryEvent): void {
  const nativeModule = NativeModules.RNSessionTelemetryWriter as
    | { pushEvent(eventJson: string): void }
    | undefined;

  if (!nativeModule) {
    return;
  }

  nativeModule.pushEvent(JSON.stringify(event));
}
