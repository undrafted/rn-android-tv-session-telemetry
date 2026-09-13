import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionTelemetry } from './index.js';

const { addListenerMock, removeMock } = vi.hoisted(() => ({
  addListenerMock: vi.fn(),
  removeMock: vi.fn(),
}));

// TVEventHandler needs a native bridge that doesn't exist outside a real RN runtime, so
// 'react-native' is mocked directly rather than pulling in RN's Jest preset here. NativeModules
// is empty (no RNSessionTelemetryWriter) so every pushEvent() exercises transferEventToNative's
// no-native-module path, same as a real JS-only test host would.
vi.mock('react-native', () => ({
  TVEventHandler: {
    addListener: addListenerMock,
  },
  NativeModules: {},
}));

function emitHardwareEvent(eventType: string): void {
  const handler = addListenerMock.mock.calls.at(-1)?.[0] as (event: { eventType: string }) => void;
  handler({ eventType });
}

// Every install() resets the clock-sync interval, so the very first pushEvent() after it always
// piggybacks a 'clock-sync' sample (see index.ts's maybeEmitClockSync) - real and intentional,
// but incidental to what these tests are actually checking, so they read the buffer through
// this rather than SessionTelemetry.getBufferedEvents() directly wherever a clock-sync sample
// would otherwise land as an unexpected extra/leading entry.
function nonClockSyncEvents() {
  return SessionTelemetry.getBufferedEvents().filter((event) => event.type !== 'clock-sync');
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
  it('mark, recordFocus, and recordDispatch are no-ops', () => {
    SessionTelemetry.mark('should-be-dropped');
    SessionTelemetry.recordFocus('should-be-dropped');
    SessionTelemetry.recordDispatch('should/be-dropped', 1);

    expect(SessionTelemetry.getBufferedEvents()).toEqual([]);
  });
});

describe('remote input', () => {
  it('records directional events as remote-input', () => {
    SessionTelemetry.install();
    emitHardwareEvent('right');

    const events = nonClockSyncEvents();
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

    const [first, second] = nonClockSyncEvents();
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

    expect(nonClockSyncEvents()).toEqual([
      expect.objectContaining({ type: 'interaction-marker', name: 'demo:card-select' }),
    ]);
  });
});

describe('recordDispatch', () => {
  it('records the action type and duration', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordDispatch('catalog/itemFocused', 4.2);

    expect(nonClockSyncEvents()).toEqual([
      expect.objectContaining({
        type: 'redux-dispatch',
        actionType: 'catalog/itemFocused',
        durationMs: 4.2,
      }),
    ]);
  });
});

describe('recordReactCommit', () => {
  it('records the profiler id, phase, and durations', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordReactCommit('CatalogRow', 'update', 12.5, 8.1);

    expect(nonClockSyncEvents()).toEqual([
      expect.objectContaining({
        type: 'react-commit',
        profilerId: 'CatalogRow',
        phase: 'update',
        actualDurationMs: 12.5,
        baseDurationMs: 8.1,
      }),
    ]);
  });
});

describe('recordFrameTiming', () => {
  it('records the frame duration', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordFrameTiming(48.2);

    expect(nonClockSyncEvents()).toEqual([
      expect.objectContaining({ type: 'frame-timing', durationMs: 48.2 }),
    ]);
  });
});

describe('bounded buffer', () => {
  it('evicts the oldest event once maxBufferedEvents is reached', () => {
    SessionTelemetry.install({ maxBufferedEvents: 2 });
    SessionTelemetry.mark('one');
    SessionTelemetry.mark('two');
    SessionTelemetry.mark('three');

    const events = SessionTelemetry.getBufferedEvents();
    expect(events).toHaveLength(2);
    expect(events.map((event) => (event as { name: string }).name)).toEqual(['two', 'three']);
    // 2, not 3 events' worth of drops: install()'s baseline clock-sync sample occupies a buffer
    // slot too (see maybeEmitClockSync) and is itself evicted first, before either 'one' is.
    expect(SessionTelemetry.getDroppedEventCount()).toBe(2);
  });

  it('resets droppedEventCount on a fresh install', () => {
    SessionTelemetry.install({ maxBufferedEvents: 1 });
    SessionTelemetry.mark('one');
    SessionTelemetry.mark('two');
    expect(SessionTelemetry.getDroppedEventCount()).toBe(2);

    SessionTelemetry.install({ maxBufferedEvents: 1 });

    expect(SessionTelemetry.getDroppedEventCount()).toBe(0);
  });

  it('clamps a non-positive maxBufferedEvents to at least 1', () => {
    SessionTelemetry.install({ maxBufferedEvents: 0 });
    SessionTelemetry.mark('one');
    SessionTelemetry.mark('two');

    expect(SessionTelemetry.getBufferedEvents()).toHaveLength(1);
  });
});

describe('clock sync', () => {
  it('emits a baseline clock-sync sample on the first push after install', () => {
    SessionTelemetry.install();
    SessionTelemetry.mark('demo:card-select');

    const events = SessionTelemetry.getBufferedEvents();
    expect(events[0]).toMatchObject({ type: 'clock-sync' });
    expect((events[0] as { wallClockUnixMs: number }).wallClockUnixMs).toEqual(expect.any(Number));
  });

  it('does not emit a second sample for a push that follows shortly after', () => {
    SessionTelemetry.install();
    SessionTelemetry.mark('one');
    SessionTelemetry.mark('two');

    const clockSyncEvents = SessionTelemetry.getBufferedEvents().filter(
      (event) => event.type === 'clock-sync',
    );
    expect(clockSyncEvents).toHaveLength(1);
  });

  it('emits a fresh baseline sample on every install, not just the first', () => {
    SessionTelemetry.install();
    SessionTelemetry.mark('previous-session-event');
    SessionTelemetry.stop();

    SessionTelemetry.install();
    SessionTelemetry.mark('new-session-event');

    const clockSyncEvents = SessionTelemetry.getBufferedEvents().filter(
      (event) => event.type === 'clock-sync',
    );
    expect(clockSyncEvents).toHaveLength(1);
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
