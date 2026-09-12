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

export interface InstallOptions {
  // Once the buffer reaches this size, the oldest event is dropped for each new one recorded
  // (a sliding window), and droppedEventCount increments — bounded memory over a multi-hour QA
  // capture (plan.md success criterion #11) instead of an unbounded array. Clamped to >= 1.
  maxBufferedEvents?: number;
}

export interface SessionTelemetryApi {
  install(options?: InstallOptions): void;
  stop(): void;
  mark(name: string): void;
  recordFocus(targetId: string): void;
  recordDispatch(actionType: string, durationMs: number): void;
  // Temporary: exposes the in-memory buffer until a native chunk writer exists (plan.md
  // Week 4 gate). Not part of the stable V1 API surface.
  getBufferedEvents(): readonly SessionTelemetryEvent[];
  // How many events the sliding window has evicted this session — the "event loss ... visible
  // in the report" disclosure success criterion #9 requires, at least on the JS side of it.
  getDroppedEventCount(): number;
}

const DEFAULT_MAX_BUFFERED_EVENTS = 10_000;

const nextSequence = createSequenceCounter();

let subscription: EventSubscription | undefined;
let previousFocusTarget: string | null = null;
let buffer: SessionTelemetryEvent[] = [];
let maxBufferedEvents = DEFAULT_MAX_BUFFERED_EVENTS;
let droppedEventCount = 0;
// Recording methods (mark/recordFocus/handleHardwareEvent) are real no-ops until install()
// runs. App code calls mark()/recordFocus() unconditionally from UI handlers rather than
// re-checking the build flag at every call site, so this is what actually keeps a disabled
// build from silently growing the buffer forever — see plan.md section 5.
let installed = false;

function pushEvent(event: SessionTelemetryEvent): void {
  if (buffer.length >= maxBufferedEvents) {
    buffer.shift();
    droppedEventCount += 1;
  }
  buffer.push(event);
}

function handleHardwareEvent(event: HWEvent): void {
  if (!installed || !isRemoteInputEventType(event.eventType)) {
    return;
  }
  pushEvent({
    type: 'remote-input',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    key: event.eventType,
  });
}

function install(options?: InstallOptions): void {
  subscription?.remove();
  buffer = [];
  droppedEventCount = 0;
  maxBufferedEvents = Math.max(1, options?.maxBufferedEvents ?? DEFAULT_MAX_BUFFERED_EVENTS);
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
  pushEvent({
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
  pushEvent({
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
  pushEvent({
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

function getDroppedEventCount(): number {
  return droppedEventCount;
}

export const SessionTelemetry: SessionTelemetryApi = {
  install,
  stop,
  mark,
  recordFocus,
  recordDispatch,
  getBufferedEvents,
  getDroppedEventCount,
};
