import { afterEach, beforeEach, expect, it, vi } from 'vitest';
import { SessionTelemetry, startStallMonitor } from './index.js';

const { pushEvent } = vi.hoisted(() => ({ pushEvent: vi.fn() }));
vi.mock('react-native', () => ({
  TVEventHandler: { addListener: () => ({ remove() {} }) },
  Platform: { OS: 'android', constants: { Model: 'test-tv' }, Version: 36 },
  NativeModules: { RNSessionTelemetryWriter: { start() {}, finish() {}, pushEvent } },
  NativeEventEmitter: class {
    addListener() {
      return { remove() {} };
    }
  },
}));

let now = 0;
beforeEach(() => {
  SessionTelemetry.stop();
  vi.useFakeTimers();
  now = 0;
  vi.spyOn(performance, 'now').mockImplementation(() => now);
  pushEvent.mockReset();
});
afterEach(() => {
  SessionTelemetry.stop();
  vi.restoreAllMocks();
  vi.useRealTimers();
});
function stalls() {
  return pushEvent.mock.calls
    .map(([json]) => JSON.parse(json as string))
    .filter((event) => event.type === 'js-stall');
}

it('does not start a timer or record before startup installation', () => {
  now = 500;
  vi.advanceTimersByTime(500);
  expect(vi.getTimerCount()).toBe(0);
  expect(stalls()).toEqual([]);
});

it('automatically transfers one metadata-only stall to native storage after startup', () => {
  SessionTelemetry.install();
  now = 300;
  vi.advanceTimersByTime(50);
  expect(stalls()).toEqual([
    { type: 'js-stall', sequence: expect.any(Number), timestamp: 300, durationMs: 250 },
  ]);
  expect(SessionTelemetry.getBufferedEvents().at(-1)).toEqual(stalls()[0]);
  expect(vi.getTimerCount()).toBe(1);
  now = 350;
  vi.advanceTimersByTime(50);
  expect(stalls()).toHaveLength(1);
});

it('uses custom interval and strict overshoot threshold, independently of wall time', () => {
  SessionTelemetry.install({ stall: { intervalMs: 100, thresholdMs: 25 } });
  vi.spyOn(Date, 'now').mockReturnValue(-1e12);
  now = 125;
  vi.advanceTimersByTime(100);
  expect(stalls()).toEqual([]);
  now = 251;
  vi.advanceTimersByTime(100);
  expect(stalls()[0].durationMs).toBe(26);
});

it('replaces timers and resets the deadline on reinstall, and leaves none after stop', () => {
  for (let i = 0; i < 20; i++) {
    SessionTelemetry.install();
    expect(vi.getTimerCount()).toBe(1);
  }
  now = 1000;
  SessionTelemetry.install();
  now = 1050;
  vi.advanceTimersByTime(50);
  expect(stalls()).toEqual([]);
  SessionTelemetry.stop();
  now = 5000;
  vi.advanceTimersByTime(500);
  expect(stalls()).toEqual([]);
  expect(vi.getTimerCount()).toBe(0);
});

it('does not duplicate a legacy manual sampler or let an old cleanup stop its replacement', () => {
  const oldStop = startStallMonitor();
  SessionTelemetry.install();
  oldStop();
  startStallMonitor();
  expect(vi.getTimerCount()).toBe(1);
  now = 300;
  vi.advanceTimersByTime(50);
  expect(stalls()).toHaveLength(1);
  SessionTelemetry.stop();
  expect(vi.getTimerCount()).toBe(0);
});

it.each([NaN, Infinity, -Infinity, -1, 0])('keeps invalid interval %s bounded', (intervalMs) => {
  SessionTelemetry.install({ stall: { intervalMs, thresholdMs: NaN } });
  now = 100;
  vi.advanceTimersByTime(100);
  expect(vi.getTimerCount()).toBe(1);
  expect(stalls().length).toBeLessThanOrEqual(1);
});

it('survives recorder failure and resumes with a new deadline', () => {
  SessionTelemetry.install();
  pushEvent.mockImplementationOnce(() => {
    throw new Error('bridge failure');
  });
  now = 300;
  expect(() => vi.advanceTimersByTime(50)).not.toThrow();
  expect(vi.getTimerCount()).toBe(1);
  now = 600;
  vi.advanceTimersByTime(50);
  expect(stalls()).toHaveLength(2);
});

it('bounds idle overhead to one callback per interval and emits no idle events', () => {
  vi.mocked(performance.now).mockRestore();
  vi.spyOn(performance, 'now').mockImplementation(() => Date.now());
  SessionTelemetry.install();
  const callsBefore = vi.mocked(performance.now).mock.calls.length;
  vi.advanceTimersByTime(60_000);
  expect(vi.mocked(performance.now).mock.calls.length - callsBefore).toBe(2400);
  expect(vi.getTimerCount()).toBe(1);
  expect(stalls()).toEqual([]);
});
