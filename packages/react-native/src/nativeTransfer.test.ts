import { beforeEach, describe, expect, it, vi } from 'vitest';

const { pushEventMock, nativeModulesMock } = vi.hoisted(() => {
  const pushEventMock = vi.fn();
  return {
    pushEventMock,
    nativeModulesMock: { RNSessionTelemetryWriter: { pushEvent: pushEventMock } } as Record<string, unknown>,
  };
});

vi.mock('react-native', () => ({
  NativeModules: nativeModulesMock,
}));

const { transferEventToNative } = await import('./nativeTransfer.js');

beforeEach(() => {
  pushEventMock.mockClear();
  nativeModulesMock.RNSessionTelemetryWriter = { pushEvent: pushEventMock };
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
