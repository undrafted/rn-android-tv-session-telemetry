import { monotonicNowMs } from './clock.js';
import { SessionTelemetry } from './index.js';

export interface StallMonitorOptions {
  // Default 50ms; clamped to at least 16ms to avoid a tight sampling loop.
  intervalMs?: number;
  // Timer overshoot must be strictly greater than this threshold. Default 50ms.
  thresholdMs?: number;
}

const DEFAULT_INTERVAL_MS = 50;
const DEFAULT_THRESHOLD_MS = 50;
const MAX_TIMER_MS = 2_147_483_647;
let stopCurrentMonitor: (() => void) | undefined;

export function stopStallMonitor(): void {
  stopCurrentMonitor?.();
}

// One timer measures lateness against a monotonic deadline, not CPU usage or exact blocking
// duration. Scheduling, GC and background suspension can also delay it. Never catch up missed
// ticks: a long delay produces one observation, then a fresh deadline.
// Kept as a standalone compatibility API; another start replaces the current sampler.
export function startStallMonitor(options: StallMonitorOptions = {}): () => void {
  stopStallMonitor();
  const intervalMs = Number.isFinite(options.intervalMs)
    ? Math.min(MAX_TIMER_MS, Math.max(16, options.intervalMs!))
    : DEFAULT_INTERVAL_MS;
  const thresholdMs = Number.isFinite(options.thresholdMs)
    ? Math.max(0, options.thresholdMs!)
    : DEFAULT_THRESHOLD_MS;
  let stopped = false;
  let deadline = monotonicNowMs() + intervalMs;
  let timer: ReturnType<typeof setTimeout>;

  function tick(): void {
    if (stopped) return;
    const overshoot = monotonicNowMs() - deadline;
    try {
      if (Number.isFinite(overshoot) && overshoot > thresholdMs) {
        SessionTelemetry.recordJsStall(overshoot);
      }
    } catch {
      // An optional telemetry bridge failure must not escape into the application timer loop.
    } finally {
      if (!stopped) {
        deadline = monotonicNowMs() + intervalMs;
        timer = setTimeout(tick, intervalMs);
      }
    }
  }

  const stop = () => {
    stopped = true;
    clearTimeout(timer);
    if (stopCurrentMonitor === stop) stopCurrentMonitor = undefined;
  };
  timer = setTimeout(tick, intervalMs);
  stopCurrentMonitor = stop;
  return stop;
}
