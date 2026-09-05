import { describe, expect, it } from 'vitest';
import { createTelemetryMiddleware } from './index.js';

describe('createTelemetryMiddleware', () => {
  it('passes actions through unchanged', () => {
    const middleware = createTelemetryMiddleware({
      includeActionTypes: true,
      includePayloads: false,
    });
    const action = { type: 'catalog/itemFocused' };
    const next = (a: unknown) => a;
    const dispatch = middleware({} as never)(next);

    expect(dispatch(action)).toBe(action);
  });
});
