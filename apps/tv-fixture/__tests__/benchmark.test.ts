/**
 * @format
 */

import { SessionTelemetry } from '@rn-android-tv-session-telemetry/react-native';
import { median, runOverheadBenchmark } from '../benchmark';

// A full replacement, not `{ ...jest.requireActual('react-native'), TVEventHandler: ... }` -
// requireActual eagerly loads FlatList's real TurboModule chain (DevMenu), which throws outside
// a native binary. SessionTelemetry.install()/mark()/stop() (the only RN library code this test
// exercises) only ever touch TVEventHandler, NativeEventEmitter, and NativeModules, so a minimal
// stub of just those three is enough - same approach packages/react-native's own Vitest suite
// uses for the same reason. TVEventHandler isn't in the RN jest preset's own mock set at all
// (it's a react-native-tvos extension, not standard RN).
jest.mock('react-native', () => ({
  TVEventHandler: { addListener: jest.fn(() => ({ remove: jest.fn() })) },
  NativeEventEmitter: jest
    .fn()
    .mockImplementation(function NativeEventEmitter() {
      return { addListener: jest.fn(() => ({ remove: jest.fn() })) };
    }),
  NativeModules: {},
  // install() reads this (index.ts's emitSessionMetadata) on every call - mirrors a realistic
  // Android shape, same as packages/react-native's own Vitest suite.
  Platform: {
    OS: 'android',
    constants: { Model: 'sdk_google_atv64_arm64' },
    Version: 14,
  },
}));

beforeEach(() => jest.useFakeTimers());
afterEach(() => {
  SessionTelemetry.stop();
  jest.useRealTimers();
});

const options = {
  rounds: 2,
  iterations: 3,
  requests: 2,
  renders: 2,
  idleMs: 10,
};

test('alternates phases, retains raw pairs and exercises asynchronous workloads', async () => {
  const render = jest.fn(async () => {});
  const request = jest.fn(async () => {});
  const pending = runOverheadBenchmark({ render, request }, options);
  await jest.runAllTimersAsync();
  const result = await pending;
  expect(result.results).toHaveLength(10);
  for (const entry of result.results) {
    expect(entry.samples.map(s => s.active)).toEqual([
      false,
      true,
      true,
      false,
    ]);
    const active = entry.samples.filter(s => s.active);
    const stopped = entry.samples.filter(s => !s.active);
    expect(entry.medianPairedDeltaMs).toBe(
      median(
        active.map((s, i) => s.perOperationMs - stopped[i].perOperationMs),
      ),
    );
  }
  const marks = result.results.find(row => row.name === 'mark')!;
  expect(
    marks.samples.map(s => s.eventCounts['interaction-marker'] ?? 0),
  ).toEqual([0, 3, 3, 0]);
  expect(request).toHaveBeenCalledTimes(12);
  expect(render).toHaveBeenCalledTimes(12);
  const before = SessionTelemetry.getBufferedEvents().length;
  SessionTelemetry.mark('after');
  expect(SessionTelemetry.getBufferedEvents()).toHaveLength(before);
  expect(jest.getTimerCount()).toBe(0);
});

test('failure stops recording and rejects instead of returning a success result', async () => {
  const pending = runOverheadBenchmark(
    {
      render: async () => {},
      request: async () => {
        throw new Error('offline');
      },
    },
    options,
  );
  const failure = pending.catch(error => error);
  await jest.runAllTimersAsync();
  expect(await failure).toEqual(new Error('offline'));
  const before = SessionTelemetry.getBufferedEvents().length;
  SessionTelemetry.mark('after');
  expect(SessionTelemetry.getBufferedEvents()).toHaveLength(before);
  expect(jest.getTimerCount()).toBe(0);
});

test('rejects invalid iteration counts before running workloads', async () => {
  await expect(
    runOverheadBenchmark({ render: async () => {} }, { iterations: 0 }),
  ).rejects.toThrow('Invalid benchmark');
});
