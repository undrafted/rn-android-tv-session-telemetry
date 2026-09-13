import { beforeEach, describe, expect, it, vi } from 'vitest';
import { SessionTelemetry } from './index.js';

const { addListenerMock, removeMock } = vi.hoisted(() => ({
  addListenerMock: vi.fn(),
  removeMock: vi.fn(),
}));

// TVEventHandler needs a native bridge that doesn't exist outside a real RN runtime, so
// 'react-native' is mocked directly rather than pulling in RN's Jest preset here. NativeModules
// is empty (no RNSessionTelemetryWriter) so every pushEvent() exercises transferEventToNative's
// no-native-module path, same as a real JS-only test host would. Platform mirrors a realistic
// Android shape since emitSessionMetadata (index.ts) reads it during every install().
vi.mock('react-native', () => ({
  TVEventHandler: {
    addListener: addListenerMock,
  },
  NativeModules: {},
  Platform: {
    OS: 'android',
    constants: { Model: 'sdk_google_atv64_arm64' },
    Version: 14,
  },
}));

function emitHardwareEvent(eventType: string): void {
  const handler = addListenerMock.mock.calls.at(-1)?.[0] as (event: { eventType: string }) => void;
  handler({ eventType });
}

// Every install() emits its own baseline 'clock-sync' sample (see index.ts's maybeEmitClockSync)
// and a one-time 'session-metadata' event (emitSessionMetadata) - both real and intentional, but
// incidental to what these tests are actually checking, so they read the buffer through this
// rather than SessionTelemetry.getBufferedEvents() directly wherever either would otherwise land
// as an unexpected extra/leading entry.
function applicationEvents() {
  return SessionTelemetry.getBufferedEvents().filter(
    (event) => event.type !== 'clock-sync' && event.type !== 'session-metadata',
  );
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

    // The fresh install's own baseline clock-sync/session-metadata events are expected here -
    // what this test actually checks is that 'previous-session-event' didn't survive the cycle.
    expect(applicationEvents()).toEqual([]);
  });
});

describe('before install', () => {
  it('mark, recordFocus, and recordDispatch are no-ops', () => {
    SessionTelemetry.mark('should-be-dropped');
    SessionTelemetry.recordFocus('should-be-dropped');
    SessionTelemetry.recordDispatch('should/be-dropped', 1);

    // applicationEvents(), not getBufferedEvents() directly: this test never installs, so it
    // adds nothing of its own, but a prior test's install() can leave its own baseline
    // clock-sync/session-metadata events sitting in the buffer (stop() doesn't clear it) - real
    // shared module state, incidental to what this test checks.
    expect(applicationEvents()).toEqual([]);
  });
});

describe('remote input', () => {
  it('records directional events as remote-input', () => {
    SessionTelemetry.install();
    emitHardwareEvent('right');

    const events = applicationEvents();
    expect(events).toHaveLength(1);
    expect(events[0]).toMatchObject({ type: 'remote-input', key: 'right' });
  });

  it('does not record focus/blur as remote-input (deprecated under New Architecture)', () => {
    SessionTelemetry.install();
    emitHardwareEvent('focus');
    emitHardwareEvent('blur');

    expect(applicationEvents()).toEqual([]);
  });
});

describe('recordFocus', () => {
  it('chains previousTargetId across successive focus changes', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordFocus('card-1');
    SessionTelemetry.recordFocus('card-2');

    const [first, second] = applicationEvents();
    expect(first).toMatchObject({ type: 'focus', targetId: 'card-1', previousTargetId: null });
    expect(second).toMatchObject({
      type: 'focus',
      targetId: 'card-2',
      previousTargetId: 'card-1',
    });
  });
});

describe('recordVisibleUpdate', () => {
  it('records the target id', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordVisibleUpdate('card-2');

    expect(applicationEvents()).toEqual([
      expect.objectContaining({ type: 'visible-update', targetId: 'card-2' }),
    ]);
  });

  it('is a no-op before install', () => {
    // Length delta, not an absolute-empty check: stop() (see beforeEach) doesn't clear the
    // buffer, so a prior test's own events can still be sitting in it here - this only checks
    // that recordVisibleUpdate itself adds nothing before install.
    const before = SessionTelemetry.getBufferedEvents().length;

    SessionTelemetry.recordVisibleUpdate('should-be-dropped');

    expect(SessionTelemetry.getBufferedEvents()).toHaveLength(before);
  });
});

describe('mark', () => {
  it('records a named interaction marker', () => {
    SessionTelemetry.install();
    SessionTelemetry.mark('demo:card-select');

    expect(applicationEvents()).toEqual([
      expect.objectContaining({ type: 'interaction-marker', name: 'demo:card-select' }),
    ]);
  });
});

describe('recordDispatch', () => {
  it('records the action type and duration', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordDispatch('catalog/itemFocused', 4.2);

    expect(applicationEvents()).toEqual([
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

    expect(applicationEvents()).toEqual([
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

    expect(applicationEvents()).toEqual([
      expect.objectContaining({ type: 'frame-timing', durationMs: 48.2 }),
    ]);
  });
});

describe('recordSelector', () => {
  it('records the selector id, duration, and change flags', () => {
    SessionTelemetry.install();
    SessionTelemetry.recordSelector('catalog/selectVisibleItemIds', 0.8, false, true);

    expect(applicationEvents()).toEqual([
      expect.objectContaining({
        type: 'selector',
        selectorId: 'catalog/selectVisibleItemIds',
        durationMs: 0.8,
        inputsChanged: false,
        resultChanged: true,
      }),
    ]);
  });

  it('is a no-op before install', () => {
    const before = SessionTelemetry.getBufferedEvents().length;

    SessionTelemetry.recordSelector('should-be-dropped', 1, true, true);

    expect(SessionTelemetry.getBufferedEvents()).toHaveLength(before);
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
    // 3, not 2 events' worth of drops: install()'s own baseline clock-sync and session-metadata
    // events each occupy a buffer slot too (see maybeEmitClockSync/emitSessionMetadata) and are
    // themselves evicted first, before either 'one' or 'two' is.
    expect(SessionTelemetry.getDroppedEventCount()).toBe(3);
  });

  it('resets droppedEventCount on a fresh install', () => {
    SessionTelemetry.install({ maxBufferedEvents: 1 });
    SessionTelemetry.mark('one');
    SessionTelemetry.mark('two');
    expect(SessionTelemetry.getDroppedEventCount()).toBe(3);

    SessionTelemetry.install({ maxBufferedEvents: 1 });

    // 1, not 0: droppedEventCount itself resets to 0 as part of install(), but with a
    // maxBufferedEvents of 1, install()'s own two baseline events (clock-sync, then
    // session-metadata) can't both fit - the second evicts the first, counting as one drop of
    // this fresh session's own event, not a carryover from the previous one. A capacity above 1
    // would show 0 here instead; this specific assertion is about the reset, not the eviction.
    expect(SessionTelemetry.getDroppedEventCount()).toBe(1);
  });

  it('clamps a non-positive maxBufferedEvents to at least 1', () => {
    SessionTelemetry.install({ maxBufferedEvents: 0 });
    SessionTelemetry.mark('one');
    SessionTelemetry.mark('two');

    expect(SessionTelemetry.getBufferedEvents()).toHaveLength(1);
  });
});

describe('session metadata', () => {
  it('emits one session-metadata event per install, with device info from Platform', () => {
    SessionTelemetry.install();

    const events = SessionTelemetry.getBufferedEvents().filter(
      (event) => event.type === 'session-metadata',
    );
    expect(events).toEqual([
      expect.objectContaining({
        deviceModel: 'sdk_google_atv64_arm64',
        osVersion: '14',
        appVersion: null,
        buildType: null,
      }),
    ]);
  });

  it('carries the appVersion/buildType the host app supplies', () => {
    SessionTelemetry.install({ appVersion: '1.2.3', buildType: 'profiling' });

    const events = SessionTelemetry.getBufferedEvents().filter(
      (event) => event.type === 'session-metadata',
    );
    expect(events).toEqual([
      expect.objectContaining({ appVersion: '1.2.3', buildType: 'profiling' }),
    ]);
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
