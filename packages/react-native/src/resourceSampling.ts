import { NativeEventEmitter, NativeModules } from 'react-native';

const EVENT_SAMPLE = 'RNSessionTelemetryResourceSampling.sample';

export interface NativeResourceSample {
  cpuUtilizationPercent: number;
  nativeHeapKb: number;
  javaHeapKb: number;
}

type ResourceSamplingNativeModule = {
  start(intervalMs: number): void;
  stop(): void;
};

let subscription: { remove(): void } | undefined;

function nativeModule(): ResourceSamplingNativeModule | undefined {
  return NativeModules.RNSessionTelemetryResourceSampling as
    ResourceSamplingNativeModule | undefined;
}

// Thin adapter onto the native ResourceSamplingModule (android/ - a HandlerThread-based
// periodic CPU/memory sampler), the same "native forwards raw notifications, JS calls the real
// recorder" pattern as globalFocus.ts. Internal to this package: unlike
// startGlobalFocusMonitor/startFrameTimingMonitor, this isn't exported from index.ts directly -
// SessionTelemetry.startResourceSampling()/stopResourceSampling() own calling it, since a
// resource-sampling window is opened/closed repeatedly through one session's lifetime rather
// than started once at app entry. A no-op when the native module isn't linked (e.g. iOS, or a
// JS-only test host), the same graceful-absence behavior as every other optional signal here.
export function startNativeResourceSampling(
  intervalMs: number,
  onSample: (sample: NativeResourceSample) => void,
): void {
  const module = nativeModule();
  if (!module) {
    return;
  }
  subscription?.remove();
  const emitter = new NativeEventEmitter(module as never);
  subscription = emitter.addListener(EVENT_SAMPLE, (event: NativeResourceSample) => {
    onSample(event);
  });
  module.start(intervalMs);
}

export function stopNativeResourceSampling(): void {
  subscription?.remove();
  subscription = undefined;
  nativeModule()?.stop();
}
