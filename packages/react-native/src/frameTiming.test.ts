import { beforeEach, describe, expect, it, vi } from 'vitest';

const { recordFrameTimingMock } = vi.hoisted(() => ({ recordFrameTimingMock: vi.fn() }));

vi.mock('./index.js', () => ({
  SessionTelemetry: { recordFrameTiming: recordFrameTimingMock },
}));

const { startMock, stopMock, addListenerMock, removeMock, nativeModulesMock } = vi.hoisted(() => {
  const startMock = vi.fn();
  const stopMock = vi.fn();
  return {
    startMock,
    stopMock,
    addListenerMock: vi.fn(),
    removeMock: vi.fn(),
    nativeModulesMock: { RNSessionTelemetryFrameTiming: { start: startMock, stop: stopMock } } as Record<
      string,
      unknown
    >,
  };
});

vi.mock('react-native', () => ({
  NativeModules: nativeModulesMock,
  // A regular function, not an arrow: `new NativeEventEmitter(...)` requires the mock
  // implementation itself to be constructable.
  NativeEventEmitter: vi.fn().mockImplementation(function FakeNativeEventEmitter() {
    return { addListener: addListenerMock.mockReturnValue({ remove: removeMock }) };
  }),
}));

const { startFrameTimingMonitor } = await import('./frameTiming.js');

function emitDelayedFrame(durationMs: number): void {
  const handler = addListenerMock.mock.calls.at(-1)?.[1] as (event: { durationMs: number }) => void;
  handler({ durationMs });
}

beforeEach(() => {
  startMock.mockClear();
  stopMock.mockClear();
  addListenerMock.mockClear();
  removeMock.mockClear();
  recordFrameTimingMock.mockClear();
  nativeModulesMock.RNSessionTelemetryFrameTiming = { start: startMock, stop: stopMock };
});

describe('startFrameTimingMonitor', () => {
  it('starts the native module with the given threshold', () => {
    startFrameTimingMonitor({ thresholdMs: 40 });

    expect(startMock).toHaveBeenCalledWith(40);
  });

  it('defaults the threshold when none is given', () => {
    startFrameTimingMonitor();

    expect(startMock).toHaveBeenCalledWith(32);
  });

  it('forwards a delayed-frame event to recordFrameTiming', () => {
    startFrameTimingMonitor();
    emitDelayedFrame(52.3);

    expect(recordFrameTimingMock).toHaveBeenCalledWith(52.3);
  });

  it('stops the native module and removes the listener when stopped', () => {
    const stop = startFrameTimingMonitor();
    stop();

    expect(removeMock).toHaveBeenCalledTimes(1);
    expect(stopMock).toHaveBeenCalledTimes(1);
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryFrameTiming = undefined;

    expect(() => startFrameTimingMonitor()()).not.toThrow();
    expect(startMock).not.toHaveBeenCalled();
  });
});
