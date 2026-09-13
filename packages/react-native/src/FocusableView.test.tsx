import { beforeEach, describe, expect, it, vi } from 'vitest';

const { addListenerMock } = vi.hoisted(() => ({
  addListenerMock: vi.fn(() => ({ remove: vi.fn() })),
}));

// Same reasoning as index.test.ts: 'react-native' needs a native bridge Vitest doesn't have.
// Pressable is never actually rendered here — FocusableView is called directly as a plain
// function and its returned element's props are inspected — so a string placeholder is enough.
// Platform mirrors a realistic Android shape since install() reads it (index.ts's
// emitSessionMetadata).
vi.mock('react-native', () => ({
  TVEventHandler: { addListener: addListenerMock },
  Pressable: 'Pressable',
  NativeModules: {},
  Platform: {
    OS: 'android',
    constants: { Model: 'sdk_google_atv64_arm64' },
    Version: 14,
  },
}));

// Real RN environments polyfill requestAnimationFrame globally; Vitest's node environment
// doesn't. Stubbed to run its callback synchronously so the visible-update assertions below
// don't need to wait a real frame.
vi.stubGlobal('requestAnimationFrame', (callback: (time: number) => void) => {
  callback(0);
  return 0;
});

const { FocusableView } = await import('./FocusableView.js');
const { SessionTelemetry } = await import('./index.js');

beforeEach(() => {
  SessionTelemetry.install();
});

describe('FocusableView', () => {
  it('records focus with the given id on onFocus', () => {
    const element = FocusableView({ id: 'card-1' });
    element.props.onFocus?.({} as never);

    // install()'s own baseline clock-sync/session-metadata events land ahead of this focus
    // event in the buffer - filtered out here since they're incidental to what this test checks.
    const events = SessionTelemetry.getBufferedEvents().filter(
      (event) => event.type !== 'clock-sync' && event.type !== 'session-metadata',
    );
    expect(events).toEqual([
      expect.objectContaining({ type: 'focus', targetId: 'card-1' }),
      expect.objectContaining({ type: 'visible-update', targetId: 'card-1' }),
    ]);
  });

  it('still calls a caller-provided onFocus handler', () => {
    const onFocus = vi.fn();
    const event = {} as never;
    const element = FocusableView({ id: 'card-1', onFocus });
    element.props.onFocus?.(event);

    expect(onFocus).toHaveBeenCalledWith(event);
  });

  it('passes other Pressable props through unchanged', () => {
    const element = FocusableView({ id: 'card-1', hasTVPreferredFocus: true });

    expect(element.type).toBe('Pressable');
    expect(element.props.hasTVPreferredFocus).toBe(true);
  });
});
