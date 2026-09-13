import { SessionTelemetry } from '@rn-session-telemetry/react-native';

export interface OverheadBenchmarkResult {
  iterations: number;
  inactiveTotalMs: number;
  inactiveAvgMs: number;
  activeTotalMs: number;
  activeAvgMs: number;
  overheadPerCallMs: number;
}

// Measures what enabling profiling actually costs a real interaction, not a theoretical
// estimate: the same SessionTelemetry.mark() call (the library's real public API - this fixture
// has no special internal access, on purpose, since that's what makes the number honest), timed
// once while the library is disabled (the no-op path every non-profiling build takes) and once
// while it's actually buffering and transferring to native. This lives in the fixture app, not
// the shipped @rn-session-telemetry/react-native package - it's a dev tool for measuring this
// project's own overhead, not something a consuming app ever needs to call, so it has no
// business in the library's public API surface. Meant to be run on-device (Hermes, the real JNI
// bridge) via RNST_BENCHMARK=1 (see index.js and the "benchmark" npm script) - a JS-test-runner
// run only checks this executes correctly, not the real number, since Node's V8 and a mocked
// native module don't reflect Hermes or a real JNI round-trip.
export function runOverheadBenchmark(iterations = 5_000): OverheadBenchmarkResult {
  const label = 'benchmark-mark';

  const inactiveStart = performance.now();
  for (let i = 0; i < iterations; i += 1) {
    SessionTelemetry.mark(label);
  }
  const inactiveTotalMs = performance.now() - inactiveStart;

  SessionTelemetry.install();
  const activeStart = performance.now();
  for (let i = 0; i < iterations; i += 1) {
    SessionTelemetry.mark(label);
  }
  const activeTotalMs = performance.now() - activeStart;
  SessionTelemetry.stop();

  const inactiveAvgMs = inactiveTotalMs / iterations;
  const activeAvgMs = activeTotalMs / iterations;

  return {
    iterations,
    inactiveTotalMs,
    inactiveAvgMs,
    activeTotalMs,
    activeAvgMs,
    overheadPerCallMs: activeAvgMs - inactiveAvgMs,
  };
}
