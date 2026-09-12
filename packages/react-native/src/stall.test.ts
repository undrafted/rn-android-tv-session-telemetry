import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';

const { recordJsStallMock } = vi.hoisted(() => ({ recordJsStallMock: vi.fn() }));

vi.mock('./index.js', () => ({
  SessionTelemetry: { recordJsStall: recordJsStallMock },
}));

const { startStallMonitor } = await import('./stall.js');

beforeEach(() => {
  vi.useFakeTimers();
  recordJsStallMock.mockClear();
});

afterEach(() => {
  vi.useRealTimers();
});

describe('startStallMonitor', () => {
  it('records a stall when a tick fires much later than expected', () => {
    const nowSpy = vi
      .spyOn(performance, 'now')
      // one call to seed `lastTick` in startStallMonitor, one call inside the first tick
      .mockReturnValueOnce(0)
      .mockReturnValueOnce(300);

    startStallMonitor({ intervalMs: 50, thresholdMs: 50 });
    vi.advanceTimersByTime(50);

    expect(recordJsStallMock).toHaveBeenCalledTimes(1);
    // elapsed 300ms - intervalMs 50ms = 250ms overshoot
    expect(recordJsStallMock).toHaveBeenCalledWith(250);

    nowSpy.mockRestore();
  });

  it('does not record a stall for normal timer jitter under the threshold', () => {
    const nowSpy = vi.spyOn(performance, 'now').mockReturnValueOnce(0).mockReturnValueOnce(55);

    startStallMonitor({ intervalMs: 50, thresholdMs: 50 });
    vi.advanceTimersByTime(50);

    expect(recordJsStallMock).not.toHaveBeenCalled();

    nowSpy.mockRestore();
  });

  it('stops scheduling further checks once the returned stop function is called', () => {
    const nowSpy = vi.spyOn(performance, 'now').mockReturnValue(0);

    const stop = startStallMonitor({ intervalMs: 50 });
    stop();
    vi.advanceTimersByTime(500);

    expect(vi.getTimerCount()).toBe(0);

    nowSpy.mockRestore();
  });
});
