import { NativeEventEmitter, NativeModules } from 'react-native';
import { SessionTelemetry } from './index.js';

const EVENT_FOCUS = 'RNSessionTelemetryGlobalFocus.focusChanged';
const EVENT_VISIBLE_UPDATE = 'RNSessionTelemetryGlobalFocus.visibleUpdate';

// Thin adapter onto the native GlobalFocusModule (android/ - ViewTreeObserver.
// OnGlobalFocusChangeListener): replaces per-component focus tracking (FocusableView, removed)
// with one app-wide listener. This file does no measurement itself, it just forwards native
// notifications into the existing recordFocus()/recordVisibleUpdate() - which still own
// sequencing, timestamps, and the previousTargetId chain, unchanged. Returns a no-op stop
// function when the native module isn't linked (e.g. iOS, or a JS-only test host), the same
// graceful-absence behavior as every other optional signal in this library.
export function startGlobalFocusMonitor(): () => void {
  const nativeModule = NativeModules.RNSessionTelemetryGlobalFocus as
    { start(): void; stop(): void } | undefined;

  if (!nativeModule) {
    return () => {};
  }

  const emitter = new NativeEventEmitter(nativeModule as never);
  const focusSubscription = emitter.addListener(EVENT_FOCUS, (event: { targetId: string }) => {
    SessionTelemetry.recordFocus(event.targetId);
  });
  const visibleUpdateSubscription = emitter.addListener(
    EVENT_VISIBLE_UPDATE,
    (event: { targetId: string }) => {
      SessionTelemetry.recordVisibleUpdate(event.targetId);
    },
  );

  nativeModule.start();

  return () => {
    focusSubscription.remove();
    visibleUpdateSubscription.remove();
    nativeModule.stop();
  };
}
