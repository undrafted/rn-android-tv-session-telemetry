import { beforeEach, describe, expect, it, vi } from 'vitest';

const { recordReactCommitMock } = vi.hoisted(() => ({ recordReactCommitMock: vi.fn() }));

vi.mock('./index.js', () => ({
  SessionTelemetry: { recordReactCommit: recordReactCommitMock },
}));

const { onProfilerRender } = await import('./profiler.js');

beforeEach(() => {
  recordReactCommitMock.mockClear();
});

describe('onProfilerRender', () => {
  it('forwards React Profiler measurements to recordReactCommit', () => {
    onProfilerRender('CatalogRow', 'mount', 12.5, 8.1, 100, 112.5);

    expect(recordReactCommitMock).toHaveBeenCalledWith(
      'CatalogRow',
      'mount',
      12.5,
      8.1,
      100,
      112.5,
    );
  });
});
