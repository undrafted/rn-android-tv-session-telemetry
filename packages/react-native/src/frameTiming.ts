import { NativeEventEmitter, NativeModules } from 'react-native';
import { SessionTelemetry } from './index.js';

export interface FrameTimingMonitorOptions {
  // Frames at or under this total duration are not forwarded from native to JS at all - at
  // 60Hz every single frame would otherwise cross the bridge, defeating the bounded JS buffer
  // (see index.ts). Default is one dropped frame's worth of budget at 60Hz.
  thresholdMs?: number;
}

const DEFAULT_THRESHOLD_MS = 32;

// Thin adapter onto the native FrameTimingModule (android/ - Window.OnFrameMetricsAvailableListener),
// matching the pattern of onProfilerRender/startStallMonitor: this file does no measurement
// itself, it just forwards native measurements into SessionTelemetry. Returns
// a no-op stop function when the native module isn't linked (e.g. iOS, or a JS-only test host),
// the same graceful-absence behavior as every other optional signal in this library.
export function startFrameTimingMonitor(options: FrameTimingMonitorOptions = {}): () => void {
  const thresholdMs = options.thresholdMs ?? DEFAULT_THRESHOLD_MS;
  const nativeModule = NativeModules.RNSessionTelemetryFrameTiming as
    | { start(thresholdMs: number): void; stop(): void }
    | undefined;

  if (!nativeModule) {
    return () => {};
  }

  const emitter = new NativeEventEmitter(nativeModule as never);
  const subscription = emitter.addListener(
    'RNSessionTelemetryFrameTiming.delayedFrame',
    (event: { durationMs: number }) => {
      SessionTelemetry.recordFrameTiming(event.durationMs);
    },
  );

  nativeModule.start(thresholdMs);

  return () => {
    subscription.remove();
    nativeModule.stop();
  };
}
