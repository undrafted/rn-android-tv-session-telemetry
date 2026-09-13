import { beforeEach, describe, expect, it, vi } from 'vitest';

const { recordFocusMock, recordVisibleUpdateMock } = vi.hoisted(() => ({
  recordFocusMock: vi.fn(),
  recordVisibleUpdateMock: vi.fn(),
}));

vi.mock('./index.js', () => ({
  SessionTelemetry: { recordFocus: recordFocusMock, recordVisibleUpdate: recordVisibleUpdateMock },
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
      RNSessionTelemetryGlobalFocus: { start: startMock, stop: stopMock },
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

const { startGlobalFocusMonitor } = await import('./globalFocus.js');

function emitEvent(eventName: string, targetId: string): void {
  const call = addListenerMock.mock.calls.find((call) => call[0] === eventName);
  const handler = call?.[1] as (event: { targetId: string }) => void;
  handler({ targetId });
}

beforeEach(() => {
  startMock.mockClear();
  stopMock.mockClear();
  addListenerMock.mockClear();
  removeMock.mockClear();
  recordFocusMock.mockClear();
  recordVisibleUpdateMock.mockClear();
  nativeModulesMock.RNSessionTelemetryGlobalFocus = { start: startMock, stop: stopMock };
});

describe('startGlobalFocusMonitor', () => {
  it('starts the native module', () => {
    startGlobalFocusMonitor();

    expect(startMock).toHaveBeenCalledTimes(1);
  });

  it('forwards a focus-changed event to recordFocus', () => {
    startGlobalFocusMonitor();
    emitEvent('RNSessionTelemetryGlobalFocus.focusChanged', 'card-2');

    expect(recordFocusMock).toHaveBeenCalledWith('card-2');
  });

  it('forwards a visible-update event to recordVisibleUpdate', () => {
    startGlobalFocusMonitor();
    emitEvent('RNSessionTelemetryGlobalFocus.visibleUpdate', 'card-2');

    expect(recordVisibleUpdateMock).toHaveBeenCalledWith('card-2');
  });

  it('stops the native module and removes both listeners when stopped', () => {
    const stop = startGlobalFocusMonitor();
    stop();

    expect(removeMock).toHaveBeenCalledTimes(2);
    expect(stopMock).toHaveBeenCalledTimes(1);
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryGlobalFocus = undefined;

    expect(() => startGlobalFocusMonitor()()).not.toThrow();
    expect(startMock).not.toHaveBeenCalled();
  });
});
