import { beforeEach, describe, expect, it, vi } from 'vitest';

const { pushEventMock, startMock, finishMock, addListenerMock, nativeModulesMock } = vi.hoisted(() => {
  const pushEventMock = vi.fn();
  const startMock = vi.fn();
  const finishMock = vi.fn();
  const addListenerMock = vi.fn();
  return {
    pushEventMock,
    startMock,
    finishMock,
    addListenerMock,
    nativeModulesMock: {
      RNSessionTelemetryWriter: { pushEvent: pushEventMock, start: startMock, finish: finishMock },
    } as Record<string, unknown>,
  };
});

// A regular function, not an arrow function - `new NativeEventEmitter(...)` requires a real
// constructor, and an explicit object return from a plain function is what `new` substitutes
// for `this`.
vi.mock('react-native', () => ({
  NativeModules: nativeModulesMock,
  NativeEventEmitter: function NativeEventEmitter() {
    return { addListener: addListenerMock };
  },
}));

const { transferEventToNative, startNativeSession, finishNativeSession, onNativeSessionOpened } =
  await import('./nativeTransfer.js');

beforeEach(() => {
  pushEventMock.mockClear();
  startMock.mockClear();
  finishMock.mockClear();
  addListenerMock.mockClear();
  nativeModulesMock.RNSessionTelemetryWriter = {
    pushEvent: pushEventMock,
    start: startMock,
    finish: finishMock,
  };
});

describe('transferEventToNative', () => {
  it('forwards the event to the native module as JSON', () => {
    transferEventToNative({
      type: 'interaction-marker',
      sequence: 1,
      timestamp: 12.5,
      name: 'demo:card-select',
    });

    expect(pushEventMock).toHaveBeenCalledWith(
      JSON.stringify({
        type: 'interaction-marker',
        sequence: 1,
        timestamp: 12.5,
        name: 'demo:card-select',
      }),
    );
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryWriter = undefined;

    expect(() =>
      transferEventToNative({
        type: 'interaction-marker',
        sequence: 1,
        timestamp: 12.5,
        name: 'demo:card-select',
      }),
    ).not.toThrow();
    expect(pushEventMock).not.toHaveBeenCalled();
  });
});

describe('startNativeSession', () => {
  it('calls the native module start()', () => {
    startNativeSession();

    expect(startMock).toHaveBeenCalledTimes(1);
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryWriter = undefined;

    expect(() => startNativeSession()).not.toThrow();
  });
});

describe('finishNativeSession', () => {
  it('calls the native module finish()', () => {
    finishNativeSession();

    expect(finishMock).toHaveBeenCalledTimes(1);
  });

  it('is a no-op when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryWriter = undefined;

    expect(() => finishNativeSession()).not.toThrow();
  });
});

describe('onNativeSessionOpened', () => {
  it('subscribes to the native sessionOpened event and unsubscribes on demand', () => {
    const removeMock = vi.fn();
    addListenerMock.mockReturnValue({ remove: removeMock });
    const callback = vi.fn();

    const unsubscribe = onNativeSessionOpened(callback);

    expect(addListenerMock).toHaveBeenCalledWith('RNSessionTelemetryWriter.sessionOpened', callback);

    unsubscribe();
    expect(removeMock).toHaveBeenCalledTimes(1);
  });

  it('returns a no-op unsubscribe when the native module is not linked', () => {
    nativeModulesMock.RNSessionTelemetryWriter = undefined;

    const unsubscribe = onNativeSessionOpened(vi.fn());

    expect(() => unsubscribe()).not.toThrow();
    expect(addListenerMock).not.toHaveBeenCalled();
  });
});
