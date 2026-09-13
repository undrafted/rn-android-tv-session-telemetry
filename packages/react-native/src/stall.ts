import { monotonicNowMs } from './clock.js';
import { SessionTelemetry } from './index.js';

export interface StallMonitorOptions {
  // How often to check the event loop, in ms. Default 50 — frequent enough to catch stalls a
  // remote-input interaction would actually notice, without being a meaningful CPU cost itself.
  intervalMs?: number;
  // Overshoot beyond intervalMs required to count as a stall, not just normal timer jitter.
  thresholdMs?: number;
}

const DEFAULT_INTERVAL_MS = 50;
const DEFAULT_THRESHOLD_MS = 50;

// Approximates JS event-loop stalls the standard way: schedule a timer for a known interval,
// and if it fires much later than expected, the gap is time the main thread spent blocked on
// something else. This is an approximation, not real profiling: a timer firing late is a
// symptom of a stall, not a measurement of what caused it (that would need CPU sampling, which
// this library deliberately doesn't do).
export function startStallMonitor(options: StallMonitorOptions = {}): () => void {
  const intervalMs = options.intervalMs ?? DEFAULT_INTERVAL_MS;
  const thresholdMs = options.thresholdMs ?? DEFAULT_THRESHOLD_MS;

  let stopped = false;
  let lastTick = monotonicNowMs();
  let timer: ReturnType<typeof setTimeout>;

  function tick(): void {
    if (stopped) {
      return;
    }
    const now = monotonicNowMs();
    const overshoot = now - lastTick - intervalMs;
    if (overshoot > thresholdMs) {
      SessionTelemetry.recordJsStall(overshoot);
    }
    lastTick = now;
    timer = setTimeout(tick, intervalMs);
  }

  timer = setTimeout(tick, intervalMs);

  return () => {
    stopped = true;
    clearTimeout(timer);
  };
}
