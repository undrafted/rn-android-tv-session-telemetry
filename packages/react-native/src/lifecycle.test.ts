import { beforeEach, describe, expect, it, vi } from 'vitest';

const { recordLifecycleTransitionMock } = vi.hoisted(() => ({
  recordLifecycleTransitionMock: vi.fn(),
}));

vi.mock('./index.js', () => ({
  SessionTelemetry: { recordLifecycleTransition: recordLifecycleTransitionMock },
}));

const { startMock, stopMock, addListenerMock, removeMock, nativeModulesMock } = vi.hoisted(() => {
  const startMock = vi.fn();
  const stopMock = vi.fn();
  return {
    startMock,
    stopMock,
    addListenerMock: vi.fn(),
    removeMock: vi.fn(),
    nativeModulesMock: {
      RNSessionTelemetryLifecycle: { start: startMock, stop: stopMock },
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

const { startLifecycleMonitor } = await import('./lifecycle.js');

function emitTransition(state: 'foreground' | 'background'): void {
  const call = addListenerMock.mock.calls.find(
    (call) => call[0] === 'RNSessionTelemetryLifecycle.transition',
  );
  const handler = call?.[1] as (event: { state: 'foreground' | 'background' }) => void;
  handler({ state });
}

beforeEach(() => {
  startMock.mockClear();
  stopMock.mockClear();
  addListenerMock.mockClear();
  removeMock.mockClear();
  recordLifecycleTransitionMock.mockClear();
  nativeModulesMock.RNSessionTelemetryLifecycle = { start: startMock, stop: stopMock };
});

describe('startLifecycleMonitor', () => {
  it('starts the native module', () => {
    startLifecycleMonitor();

    expect(startMock).toHaveBeenCalledTimes(1);
  });

  it('forwards a foreground transition to recordLifecycleTransition', () => {
    startLifecycleMonitor();
    emitTransition('foreground');

    expect(recordLifecycleTransitionMock).toHaveBeenCalledWith('foreground');
  });

  it('forwards a background transition to recordLifecycleTransition', () => {
    startLifecycleMonitor();
    emitTransition('background');

    expect(recordLifecycleTransitionMock).toHaveBeenCalledWith('background');
  });

  it('stops the native module and removes the listener when stopped', () => {
    const stop = startLifecycleMonitor();
    stop();

    expect(removeMock).toHaveBeenCalledTimes(1);
    expect(stopMock).toHaveBeenCalledTimes(1);
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryLifecycle = undefined;

    expect(() => startLifecycleMonitor()()).not.toThrow();
    expect(startMock).not.toHaveBeenCalled();
  });
});
