import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mocking '@rn-session-telemetry/react-native' directly (rather than trying to mock the
// 'react-native' it transitively imports) — Vitest doesn't reliably intercept transitive
// imports across a workspace symlink boundary, and this is a more focused unit test of the
// middleware's own logic anyway.
const { recordDispatchMock } = vi.hoisted(() => ({ recordDispatchMock: vi.fn() }));

vi.mock('@rn-session-telemetry/react-native', () => ({
  SessionTelemetry: { recordDispatch: recordDispatchMock },
}));

const { createTelemetryMiddleware } = await import('./index.js');

beforeEach(() => {
  recordDispatchMock.mockClear();
});

function dispatchThroughMiddleware(
  options: Parameters<typeof createTelemetryMiddleware>[0],
  action: unknown,
) {
  const middleware = createTelemetryMiddleware(options);
  const next = (a: unknown) => a;
  return middleware({} as never)(next)(action);
}

describe('createTelemetryMiddleware', () => {
  it('passes actions through unchanged', () => {
    const action = { type: 'catalog/itemFocused' };

    expect(
      dispatchThroughMiddleware({ includeActionTypes: true, includePayloads: false }, action),
    ).toBe(action);
  });

  it('records the action type and a duration when includeActionTypes is true', () => {
    dispatchThroughMiddleware(
      { includeActionTypes: true, includePayloads: false },
      { type: 'catalog/itemFocused' },
    );

    expect(recordDispatchMock).toHaveBeenCalledTimes(1);
    const [actionType, durationMs] = recordDispatchMock.mock.calls[0] as [string, number];
    expect(actionType).toBe('catalog/itemFocused');
    expect(durationMs).toBeGreaterThanOrEqual(0);
  });

  it('records a generic action type when includeActionTypes is false', () => {
    dispatchThroughMiddleware(
      { includeActionTypes: false, includePayloads: false },
      { type: 'catalog/itemFocused' },
    );

    expect(recordDispatchMock).toHaveBeenCalledWith('redux/action', expect.any(Number));
  });

  it('ignores actions without a type', () => {
    dispatchThroughMiddleware({ includeActionTypes: true, includePayloads: false }, 'not-an-fsa');

    expect(recordDispatchMock).not.toHaveBeenCalled();
  });

  it('still records the dispatch when a downstream reducer throws, and rethrows', () => {
    const middleware = createTelemetryMiddleware({
      includeActionTypes: true,
      includePayloads: false,
    });
    const next = () => {
      throw new Error('reducer exploded');
    };
    const action = { type: 'catalog/itemFocused' };

    expect(() => middleware({} as never)(next)(action)).toThrow('reducer exploded');
    expect(recordDispatchMock).toHaveBeenCalledWith('catalog/itemFocused', expect.any(Number));
  });
});
