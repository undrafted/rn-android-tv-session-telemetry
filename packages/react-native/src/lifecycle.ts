import { NativeEventEmitter, NativeModules } from 'react-native';
import { SessionTelemetry } from './index.js';

const EVENT_TRANSITION = 'RNSessionTelemetryLifecycle.transition';

// Thin adapter onto the native LifecycleModule (android/ - Application.
// ActivityLifecycleCallbacks started/stopped counts) - same "native forwards raw notifications,
// JS calls the real recorder" pattern as globalFocus.ts/frameTiming.ts. Unlike those two, this
// is started automatically from index.ts's install() - a foreground/background transition needs
// no application call to opt in, the same self-contained tier as network/JS-stall capture.
// Returns a no-op stop function when the native module isn't linked (e.g. iOS, or a JS-only test
// host), the same graceful-absence behavior as every other optional signal in this library.
export function startLifecycleMonitor(): () => void {
  const nativeModule = NativeModules.RNSessionTelemetryLifecycle as
    { start(): void; stop(): void } | undefined;

  if (!nativeModule) {
    return () => {};
  }

  const emitter = new NativeEventEmitter(nativeModule as never);
  const subscription = emitter.addListener(
    EVENT_TRANSITION,
    (event: { state: 'foreground' | 'background' }) => {
      SessionTelemetry.recordLifecycleTransition(event.state);
    },
  );

  nativeModule.start();

  return () => {
    subscription.remove();
    nativeModule.stop();
  };
}
