/**
 * @format
 */

import { SessionTelemetry } from '@rn-session-telemetry/react-native';
import { runOverheadBenchmark } from '../benchmark';

// A full replacement, not `{ ...jest.requireActual('react-native'), TVEventHandler: ... }` -
// requireActual eagerly loads FlatList's real TurboModule chain (DevMenu), which throws outside
// a native binary. SessionTelemetry.install()/mark()/stop() (the only RN library code this test
// exercises) only ever touch TVEventHandler, NativeEventEmitter, and NativeModules, so a minimal
// stub of just those three is enough - same approach packages/react-native's own Vitest suite
// uses for the same reason. TVEventHandler isn't in the RN jest preset's own mock set at all
// (it's a react-native-tvos extension, not standard RN).
jest.mock('react-native', () => ({
  TVEventHandler: { addListener: jest.fn(() => ({ remove: jest.fn() })) },
  NativeEventEmitter: jest.fn().mockImplementation(function NativeEventEmitter() {
    return { addListener: jest.fn(() => ({ remove: jest.fn() })) };
  }),
  NativeModules: {},
}));

test('runs the requested number of iterations in both phases', () => {
  const result = runOverheadBenchmark(50);

  expect(result.iterations).toBe(50);
  expect(result.inactiveTotalMs).toBeGreaterThanOrEqual(0);
  expect(result.activeTotalMs).toBeGreaterThanOrEqual(0);
  expect(Number.isFinite(result.inactiveAvgMs)).toBe(true);
  expect(Number.isFinite(result.activeAvgMs)).toBe(true);
  expect(result.overheadPerCallMs).toBe(result.activeAvgMs - result.inactiveAvgMs);
});

test('leaves the library stopped afterward, not still installed', () => {
  runOverheadBenchmark(10);
  const bufferedBefore = SessionTelemetry.getBufferedEvents().length;

  // mark() is a no-op once stopped - a later call shouldn't grow the buffer at all.
  SessionTelemetry.mark('after-benchmark');

  expect(SessionTelemetry.getBufferedEvents().length).toBe(bufferedBefore);
});

test('defaults to 5000 iterations when none is given', () => {
  const result = runOverheadBenchmark();

  expect(result.iterations).toBe(5_000);
});
