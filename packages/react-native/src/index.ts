import { TVEventHandler, type EventSubscription, type HWEvent } from 'react-native';
import { createSequenceCounter, monotonicNowMs } from './clock.js';
import { isRemoteInputEventType, type SessionTelemetryEvent } from './events.js';

export type {
  SessionTelemetryEvent,
  RemoteInputEvent,
  FocusEvent,
  InteractionMarkerEvent,
  ReduxDispatchEvent,
} from './events.js';
export { FocusableView, type FocusableViewProps } from './FocusableView.js';

export interface SessionTelemetryApi {
  install(): void;
  stop(): void;
  mark(name: string): void;
  recordFocus(targetId: string): void;
  recordDispatch(actionType: string, durationMs: number): void;
  // Temporary: exposes the in-memory buffer until a native chunk writer exists (plan.md
  // Week 4 gate). Not part of the stable V1 API surface.
  getBufferedEvents(): readonly SessionTelemetryEvent[];
}

const nextSequence = createSequenceCounter();

let subscription: EventSubscription | undefined;
let previousFocusTarget: string | null = null;
let buffer: SessionTelemetryEvent[] = [];
// Recording methods (mark/recordFocus/handleHardwareEvent) are real no-ops until install()
// runs. App code calls mark()/recordFocus() unconditionally from UI handlers rather than
// re-checking the build flag at every call site, so this is what actually keeps a disabled
// build from silently growing the buffer forever — see plan.md section 5.
let installed = false;

function handleHardwareEvent(event: HWEvent): void {
  if (!installed || !isRemoteInputEventType(event.eventType)) {
    return;
  }
  buffer.push({
    type: 'remote-input',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    key: event.eventType,
  });
}

function install(): void {
  subscription?.remove();
  buffer = [];
  previousFocusTarget = null;
  installed = true;
  subscription = TVEventHandler.addListener(handleHardwareEvent);
}

function stop(): void {
  installed = false;
  subscription?.remove();
  subscription = undefined;
}

function mark(name: string): void {
  if (!installed) {
    return;
  }
  buffer.push({
    type: 'interaction-marker',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    name,
  });
}

function recordFocus(targetId: string): void {
  if (!installed) {
    return;
  }
  buffer.push({
    type: 'focus',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    targetId,
    previousTargetId: previousFocusTarget,
  });
  previousFocusTarget = targetId;
}

function recordDispatch(actionType: string, durationMs: number): void {
  if (!installed) {
    return;
  }
  buffer.push({
    type: 'redux-dispatch',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    actionType,
    durationMs,
  });
}

function getBufferedEvents(): readonly SessionTelemetryEvent[] {
  return buffer;
}

export const SessionTelemetry: SessionTelemetryApi = {
  install,
  stop,
  mark,
  recordFocus,
  recordDispatch,
  getBufferedEvents,
};
