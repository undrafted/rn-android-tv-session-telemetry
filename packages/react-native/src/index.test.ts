import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionTelemetry } from './index.js';

const { addListenerMock, removeMock } = vi.hoisted(() => ({
  addListenerMock: vi.fn(),
  removeMock: vi.fn(),
}));

// TVEventHandler needs a native bridge that doesn't exist outside a real RN runtime, so
// 'react-native' is mocked directly rather than pulling in RN's Jest preset here.
vi.mock('react-native', () => ({
  TVEventHandler: {
    addListener: addListenerMock,
  },
}));

function emitHardwareEvent(eventType: string): void {
  const handler = addListenerMock.mock.calls.at(-1)?.[0] as (event: { eventType: string }) => void;
  handler({ eventType });
}

beforeEach(() => {
  SessionTelemetry.stop();
  addListenerMock.mockClear();
  removeMock.mockClear();
  addListenerMock.mockReturnValue({ remove: removeMock });
});

describe('install/stop', () => {
  it('subscribes to TVEventHandler on install', () => {
    SessionTelemetry.install();

    expect(addListenerMock).toHaveBeenCalledTimes(1);
  });

  it('removes the subscription on stop', () => {
    SessionTelemetry.install();
    SessionTelemetry.stop();

    expect(removeMock).toHaveBeenCalledTimes(1);
  });

  it('resets the buffer from a previous install/stop cycle', () => {
    SessionTelemetry.install();
    SessionTelemetry.mark('previous-session-event');
    SessionTelemetry.stop();

    SessionTelemetry.install();

    expect(SessionTelemetry.getBufferedEvents()).toEqual([]);
  });
});

describe('before install', () => {
  it('mark and recordFocus are no-ops', () => {
    SessionTelemetry.mark('should-be-dropped');
    SessionTelemetry.recordFocus('should-be-dropped');

    expect(SessionTelemetry.getBufferedEvents()).toEqual([]);
  });
});

describe('remote input', () => {
  it('records directional events as remote-input', () => {
    SessionTelemetry.install();
    emitHardwareEvent('right');

    const events = SessionTelemetry.getBufferedEvents();
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ type: 'remote-input', key: 'right' });
  });

  it('does not record focus/blur as remote-input (deprecated under New Architecture)', () => {
    SessionTelemetry.install();
    emitHardwareEvent('focus');
    emitHardwareEvent('blur');

    expect(SessionTelemetry.getBufferedEvents()).toEqual([]);
  });
});

describe('recordFocus', () => {
  it('chains previousTargetId across successive focus changes', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordFocus('card-1');
    SessionTelemetry.recordFocus('card-2');

    const [first, second] = SessionTelemetry.getBufferedEvents();
    expect(first).toMatchObject({ type: 'focus', targetId: 'card-1', previousTargetId: null });
    expect(second).toMatchObject({
      type: 'focus',
      targetId: 'card-2',
      previousTargetId: 'card-1',
    });
  });
});

describe('mark', () => {
  it('records a named interaction marker', () => {
    SessionTelemetry.install();
    SessionTelemetry.mark('demo:card-select');

    expect(SessionTelemetry.getBufferedEvents()).toEqual([
      expect.objectContaining({ type: 'interaction-marker', name: 'demo:card-select' }),
    ]);
  });
});

describe('sequence numbers', () => {
  it('increase monotonically across mixed event types', () => {
    SessionTelemetry.install();
    emitHardwareEvent('up');
    SessionTelemetry.recordFocus('card-1');
    SessionTelemetry.mark('demo:card-select');

    const sequences = SessionTelemetry.getBufferedEvents().map((event) => event.sequence);
    expect(sequences).toEqual([...sequences].sort((a, b) => a - b));
    expect(new Set(sequences).size).toBe(sequences.length);
  });
});
