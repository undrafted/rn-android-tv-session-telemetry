import { beforeEach, describe, expect, it, vi } from 'vitest';

// Mocking '@rn-session-telemetry/react-native' directly (rather than trying to mock the
// 'react-native' it transitively imports) — Vitest doesn't reliably intercept transitive
// imports across a workspace symlink boundary, and this is a more focused unit test of the
// middleware's own logic anyway.
const { recordDispatchMock, recordSelectorMock } = vi.hoisted(() => ({
  recordDispatchMock: vi.fn(),
  recordSelectorMock: vi.fn(),
}));

vi.mock('@rn-session-telemetry/react-native', () => ({
  SessionTelemetry: { recordDispatch: recordDispatchMock, recordSelector: recordSelectorMock },
}));

const { createTelemetryMiddleware, telemetrySelector } = await import('./index.js');

beforeEach(() => {
  recordDispatchMock.mockClear();
  recordSelectorMock.mockClear();
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

describe('telemetrySelector', () => {
  it('returns the exact result the wrapped selector returns, unchanged', () => {
    const result = { id: 'card-1' };
    const selector = telemetrySelector('catalog/selectFocused', () => result);

    expect(selector()).toBe(result);
  });

  it('calls the wrapped selector exactly once per invocation', () => {
    const spy = vi.fn(() => 'value');
    const selector = telemetrySelector('catalog/selectFocused', spy);

    selector();
    selector();

    expect(spy).toHaveBeenCalledTimes(2);
  });

  it('reports the first call as changed inputs and a changed result', () => {
    const selector = telemetrySelector('catalog/selectFocused', () => ({}));

    selector();

    expect(recordSelectorMock).toHaveBeenCalledWith(
      'catalog/selectFocused',
      expect.any(Number),
      true,
      true,
    );
  });

  it('reports unchanged inputs when the same argument references repeat', () => {
    const state = { catalog: { items: [] } };
    const selector = telemetrySelector('catalog/selectFocused', (s: typeof state) => s);

    selector(state);
    recordSelectorMock.mockClear();
    selector(state);

    const [, , inputsChanged] = recordSelectorMock.mock.calls[0] as [
      string,
      number,
      boolean,
      boolean,
    ];
    expect(inputsChanged).toBe(false);
  });

  it('reports changed inputs when an argument reference differs', () => {
    const selector = telemetrySelector('catalog/selectFocused', (s: unknown) => s);

    selector({ a: 1 });
    recordSelectorMock.mockClear();
    selector({ a: 1 });

    const [, , inputsChanged] = recordSelectorMock.mock.calls[0] as [
      string,
      number,
      boolean,
      boolean,
    ];
    expect(inputsChanged).toBe(true);
  });

  it('reports an unstable result: unchanged inputs but a new result reference', () => {
    const state = { items: [1, 2, 3] };
    // A classic unstable selector - .map() returns a fresh array every call even though
    // `state.items` itself never changes reference.
    const selectIds = (s: typeof state) => s.items.map((n) => n);
    const selector = telemetrySelector('catalog/selectIds', selectIds);

    selector(state);
    recordSelectorMock.mockClear();
    selector(state);

    expect(recordSelectorMock).toHaveBeenCalledWith(
      'catalog/selectIds',
      expect.any(Number),
      false,
      true,
    );
  });

  it('reports a stable result when inputs and the returned reference are both unchanged', () => {
    const state = { count: 1 };
    const stableResult = { doubled: 2 };
    const selector = telemetrySelector('catalog/selectDoubled', (_s: typeof state) => stableResult);

    selector(state);
    recordSelectorMock.mockClear();
    selector(state);

    expect(recordSelectorMock).toHaveBeenCalledWith(
      'catalog/selectDoubled',
      expect.any(Number),
      false,
      false,
    );
  });
});
