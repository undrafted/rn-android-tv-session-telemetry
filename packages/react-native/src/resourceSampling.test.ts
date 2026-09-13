import { beforeEach, describe, expect, it, vi } from 'vitest';

const { startMock, stopMock, addListenerMock, removeMock, nativeModulesMock } = vi.hoisted(() => {
  const startMock = vi.fn();
  const stopMock = vi.fn();
  return {
    startMock,
    stopMock,
    addListenerMock: vi.fn(),
    removeMock: vi.fn(),
    nativeModulesMock: {
      RNSessionTelemetryResourceSampling: { start: startMock, stop: stopMock },
    } as Record<string, unknown>,
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

const { startNativeResourceSampling, stopNativeResourceSampling } =
  await import('./resourceSampling.js');

function emitSample(sample: {
  cpuUtilizationPercent: number;
  nativeHeapKb: number;
  javaHeapKb: number;
}): void {
  const call = addListenerMock.mock.calls.find(
    (call) => call[0] === 'RNSessionTelemetryResourceSampling.sample',
  );
  const handler = call?.[1] as (event: typeof sample) => void;
  handler(sample);
}

beforeEach(() => {
  // Resets this module's own internal subscription state left over from a previous test -
  // stopNativeResourceSampling is idempotent, so this is safe even when nothing was started.
  stopNativeResourceSampling();
  startMock.mockClear();
  stopMock.mockClear();
  addListenerMock.mockClear();
  removeMock.mockClear();
  nativeModulesMock.RNSessionTelemetryResourceSampling = { start: startMock, stop: stopMock };
});

describe('startNativeResourceSampling', () => {
  it('starts the native module with the given interval', () => {
    startNativeResourceSampling(500, () => {});

    expect(startMock).toHaveBeenCalledWith(500);
  });

  it('forwards a native sample to the callback', () => {
    const onSample = vi.fn();
    startNativeResourceSampling(500, onSample);

    emitSample({ cpuUtilizationPercent: 42.5, nativeHeapKb: 1000, javaHeapKb: 2000 });

    expect(onSample).toHaveBeenCalledWith({
      cpuUtilizationPercent: 42.5,
      nativeHeapKb: 1000,
      javaHeapKb: 2000,
    });
  });

  it('removes the previous listener when started again without stopping', () => {
    startNativeResourceSampling(500, () => {});
    startNativeResourceSampling(1000, () => {});

    expect(removeMock).toHaveBeenCalledTimes(1);
    expect(startMock).toHaveBeenCalledTimes(2);
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryResourceSampling = undefined;

    expect(() => startNativeResourceSampling(500, () => {})).not.toThrow();
    expect(startMock).not.toHaveBeenCalled();
  });
});

describe('stopNativeResourceSampling', () => {
  it('stops the native module and removes the listener', () => {
    startNativeResourceSampling(500, () => {});
    stopNativeResourceSampling();

    expect(removeMock).toHaveBeenCalledTimes(1);
    expect(stopMock).toHaveBeenCalledTimes(1);
  });

  it('is a no-op when nothing was started', () => {
    expect(() => stopNativeResourceSampling()).not.toThrow();
    expect(stopMock).toHaveBeenCalledTimes(1);
  });
});
