import { Platform } from 'react-native';
import {
  SessionTelemetry,
  onProfilerRender,
} from '@rn-android-tv-session-telemetry/react-native';
import {
  createTelemetryMiddleware,
  telemetrySelector,
} from '@rn-android-tv-session-telemetry/redux';
import { applyMiddleware, createStore } from 'redux';

export const BENCHMARK_DEFAULTS = {
  rounds: 5,
  iterations: 500,
  requests: 10,
  renders: 20,
  idleMs: 500,
  // How long each resource-sampling overhead phase waits, separate from idleMs above so this
  // case's own idle window can be tuned independently (a resource-sampling window needs to stay
  // open long enough to take several samples at its own interval, not just wait once).
  resourceSamplingIdleMs: 2000,
};
export type BenchmarkOptions = Partial<typeof BENCHMARK_DEFAULTS>;
export interface BenchmarkSample {
  active: boolean;
  round: number;
  totalMs: number;
  perOperationMs: number;
  recordedEvents: number;
  eventCounts: Record<string, number>;
}
export interface BenchmarkCaseResult {
  name: string;
  unit: string;
  samples: BenchmarkSample[];
  stoppedMedianMs: number;
  activeMedianMs: number;
  medianPairedDeltaMs: number;
}
export interface BenchmarkDrivers {
  render: (profiled: boolean, iteration: number) => Promise<void>;
  request?: () => Promise<void>;
}

export function median(values: number[]): number {
  const sorted = [...values].sort((a, b) => a - b);
  const middle = Math.floor(sorted.length / 2);
  return sorted.length % 2
    ? sorted[middle]!
    : (sorted[middle - 1]! + sorted[middle]!) / 2;
}

const pause = (ms: number) =>
  new Promise<void>(resolve => setTimeout(resolve, ms));

async function localRequest(): Promise<void> {
  const controller = new AbortController();
  const timeout = setTimeout(() => controller.abort(), 5000);
  try {
    const response = await fetch(
      'http://127.0.0.1:8787/items?token=benchmark-secret',
      { signal: controller.signal },
    );
    if (!response.ok)
      throw new Error(`Benchmark endpoint returned HTTP ${response.status}`);
    await response.text();
  } finally {
    clearTimeout(timeout);
  }
}

// JS elapsed time includes submission to native, not completion of native writes.
// Both phases use the same profiling-enabled binary, not separate production builds.
export async function runOverheadBenchmark(
  drivers: BenchmarkDrivers,
  options: BenchmarkOptions = {},
) {
  const config = { ...BENCHMARK_DEFAULTS, ...options };
  for (const [key, value] of Object.entries(config)) {
    if (!Number.isInteger(value) || value < 1 || value > 10000)
      throw new Error(`Invalid benchmark ${key}`);
  }
  const results: BenchmarkCaseResult[] = [];
  const reducer = (state = 0) => state + 1;
  const store = createStore(
    reducer,
    applyMiddleware(
      createTelemetryMiddleware({
        includeActionTypes: true,
        includePayloads: false,
      }),
    ),
  );
  const select = telemetrySelector(
    'benchmark-selector',
    (state: number) => state,
  );
  const cases = [
    {
      name: 'mark',
      unit: 'ms/call',
      count: config.iterations,
      operation: () => SessionTelemetry.mark('benchmark'),
    },
    {
      name: 'redux-dispatch',
      unit: 'ms/call',
      count: config.iterations,
      operation: () => {
        store.dispatch({ type: 'benchmark/tick' });
      },
    },
    {
      name: 'redux-selector',
      unit: 'ms/call',
      count: config.iterations,
      operation: () => {
        select(1);
      },
    },
    {
      name: 'stall-event',
      unit: 'ms/call',
      count: config.iterations,
      operation: () => SessionTelemetry.recordJsStall(60),
    },
    {
      name: 'react-callback',
      unit: 'ms/call',
      count: config.iterations,
      operation: () => {
        const now = performance.now();
        onProfilerRender('benchmark', 'update', 1, 1, now - 1, now);
      },
    },
  ];
  async function measure(
    name: string,
    unit: string,
    count: number,
    work: (active: boolean) => void | Promise<void>,
    warmup: (active: boolean) => void | Promise<void>,
  ) {
    const samples: BenchmarkSample[] = [];
    for (let round = 0; round < config.rounds; round += 1) {
      for (const active of round % 2 ? [true, false] : [false, true]) {
        SessionTelemetry.stop();
        if (active) SessionTelemetry.install({ buildType: 'benchmark' });
        try {
          await warmup(active);
          await pause(50);
          const cutoff =
            SessionTelemetry.getBufferedEvents().at(-1)?.sequence ?? -1;
          const start = performance.now();
          await work(active);
          const totalMs = performance.now() - start;
          const recorded = SessionTelemetry.getBufferedEvents().filter(
            event => event.sequence > cutoff,
          );
          const eventCounts: Record<string, number> = {};
          for (const event of recorded)
            eventCounts[event.type] = (eventCounts[event.type] ?? 0) + 1;
          samples.push({
            active,
            round,
            totalMs,
            perOperationMs: totalMs / count,
            recordedEvents: recorded.length,
            eventCounts,
          });
        } finally {
          SessionTelemetry.stop();
        }
        await pause(50);
      }
    }
    const phase = (active: boolean) =>
      samples.filter(s => s.active === active).map(s => s.perOperationMs);
    const active = phase(true),
      stopped = phase(false);
    results.push({
      name,
      unit,
      samples,
      stoppedMedianMs: median(stopped),
      activeMedianMs: median(active),
      medianPairedDeltaMs: median(active.map((v, i) => v - stopped[i]!)),
    });
  }
  // Every other case above measures JS-side call cost, captured by wall-clock timing around a
  // synchronous operation - that technique can't see a resource-sampling window's cost, which is
  // real work on a native HandlerThread, off the JS thread entirely, never blocking any JS call.
  // This measures a different, real-world question instead: does an open window make ordinary
  // JS scheduling less responsive, the same way idle-timer above measures lateness rather than a
  // call duration. This is scheduling-contention overhead, not the sampler thread's own CPU
  // time - a device without free cores to spare could show contention this technique can't
  // separate from ordinary system noise.
  async function measureResourceSamplingWindow(
    intervalMs: number,
  ): Promise<BenchmarkCaseResult> {
    const samples: BenchmarkSample[] = [];
    SessionTelemetry.install({ buildType: 'benchmark' });
    try {
      for (let round = 0; round < config.rounds; round += 1) {
        for (const samplingOn of round % 2 ? [true, false] : [false, true]) {
          if (samplingOn)
            SessionTelemetry.startResourceSampling({ intervalMs });
          await pause(50);
          const cutoff =
            SessionTelemetry.getBufferedEvents().at(-1)?.sequence ?? -1;
          const start = performance.now();
          await pause(config.resourceSamplingIdleMs);
          const totalMs =
            performance.now() - start - config.resourceSamplingIdleMs;
          const recorded = SessionTelemetry.getBufferedEvents().filter(
            event => event.sequence > cutoff,
          );
          const eventCounts: Record<string, number> = {};
          for (const event of recorded)
            eventCounts[event.type] = (eventCounts[event.type] ?? 0) + 1;
          samples.push({
            active: samplingOn,
            round,
            totalMs,
            perOperationMs: totalMs,
            recordedEvents: recorded.length,
            eventCounts,
          });
          if (samplingOn) SessionTelemetry.stopResourceSampling();
          await pause(50);
        }
      }
    } finally {
      SessionTelemetry.stop();
    }
    const phase = (active: boolean) =>
      samples.filter(s => s.active === active).map(s => s.perOperationMs);
    const samplingOnPhase = phase(true),
      samplingOffPhase = phase(false);
    return {
      name: `resource-sampling-window-${intervalMs}ms`,
      unit: 'ms timer lateness beyond requested wait',
      samples,
      stoppedMedianMs: median(samplingOffPhase),
      activeMedianMs: median(samplingOnPhase),
      medianPairedDeltaMs: median(
        samplingOnPhase.map((v, i) => v - samplingOffPhase[i]!),
      ),
    };
  }
  try {
    for (const entry of cases) {
      const loop = () => {
        for (let i = 0; i < entry.count; i += 1) entry.operation();
      };
      await measure(entry.name, entry.unit, entry.count, loop, loop);
    }
    const request = drivers.request ?? localRequest;
    await measure(
      'network-fetch',
      'ms/request including transport',
      config.requests,
      async () => {
        for (let i = 0; i < config.requests; i += 1) await request();
      },
      () => request(),
    );
    await measure(
      'idle-timer',
      'ms beyond requested wait',
      1,
      () => pause(config.idleMs),
      () => pause(100),
    );
    const idle = results[results.length - 1]!;
    for (const sample of idle.samples) {
      sample.totalMs -= config.idleMs;
      sample.perOperationMs -= config.idleMs;
    }
    idle.stoppedMedianMs -= config.idleMs;
    idle.activeMedianMs -= config.idleMs;
    await measure(
      'react-render',
      'ms/update through layout effect',
      config.renders,
      async active => {
        for (let i = 0; i < config.renders; i += 1)
          await drivers.render(active, i);
      },
      active => drivers.render(active, -1),
    );
    results.push(await measureResourceSamplingWindow(500));
    results.push(await measureResourceSamplingWindow(16));
    return {
      config,
      device: {
        model: Platform.OS === 'android' ? Platform.constants.Model : 'unknown',
        os: Platform.OS,
        version: Platform.Version,
      },
      results,
      limits: [
        'Stopped baseline retains Redux wrappers.',
        'Both phases use the profiling renderer; this does not measure its overhead versus production.',
        'Native write completion, focus and frame listener overhead are not measured.',
        'Idle-timer and resource-sampling-window results measure timer lateness, not sampler CPU cost directly - a device without free cores to spare could show contention this technique cannot separate from ordinary system noise.',
        'Network results include local transport and scheduling noise; negative deltas are retained.',
      ],
    };
  } finally {
    SessionTelemetry.stop();
  }
}
