import { describe, expect, it } from 'vitest';
import { SessionTelemetry } from './index.js';

describe('SessionTelemetry no-op implementation', () => {
  it('install, mark, and stop do not throw', () => {
    expect(() => SessionTelemetry.install()).not.toThrow();
    expect(() => SessionTelemetry.mark('interaction:start')).not.toThrow();
    expect(() => SessionTelemetry.stop()).not.toThrow();
  });
});
