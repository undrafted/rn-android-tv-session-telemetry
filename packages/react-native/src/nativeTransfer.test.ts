import { beforeEach, describe, expect, it, vi } from 'vitest';

const { pushEventMock, startMock, finishMock, nativeModulesMock } = vi.hoisted(() => {
  const pushEventMock = vi.fn();
  const startMock = vi.fn();
  const finishMock = vi.fn();
  return {
    pushEventMock,
    startMock,
    finishMock,
    nativeModulesMock: {
      RNSessionTelemetryWriter: { pushEvent: pushEventMock, start: startMock, finish: finishMock },
    } as Record<string, unknown>,
  };
});

vi.mock('react-native', () => ({
  NativeModules: nativeModulesMock,
}));

const { transferEventToNative, startNativeSession, finishNativeSession } = await import('./nativeTransfer.js');

beforeEach(() => {
  pushEventMock.mockClear();
  startMock.mockClear();
  finishMock.mockClear();
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
