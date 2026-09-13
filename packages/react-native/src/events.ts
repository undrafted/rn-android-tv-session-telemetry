export interface RemoteInputEvent {
  type: 'remote-input';
  sequence: number;
  timestamp: number;
  key: string;
}

export interface FocusEvent {
  type: 'focus';
  sequence: number;
  timestamp: number;
  targetId: string;
  previousTargetId: string | null;
}

export interface InteractionMarkerEvent {
  type: 'interaction-marker';
  sequence: number;
  timestamp: number;
  name: string;
}

export interface ReduxDispatchEvent {
  type: 'redux-dispatch';
  sequence: number;
  timestamp: number;
  actionType: string;
  durationMs: number;
}

export interface NetworkEvent {
  type: 'network';
  sequence: number;
  timestamp: number;
  method: string;
  url: string;
  status: number;
  durationMs: number;
  requestBytes: number | null;
  responseBytes: number | null;
}

export interface JsStallEvent {
  type: 'js-stall';
  sequence: number;
  timestamp: number;
  durationMs: number;
}

export interface ReactCommitEvent {
  type: 'react-commit';
  sequence: number;
  timestamp: number;
  // React's own Profiler onRender identifiers/measurements, passed through unchanged: the
  // public Profiler API is the documented hook, not an internal one that could change under us.
  profilerId: string;
  phase: 'mount' | 'update' | 'nested-update';
  actualDurationMs: number;
  baseDurationMs: number;
}

export interface FrameTimingEvent {
  type: 'frame-timing';
  sequence: number;
  timestamp: number;
  durationMs: number;
}

// A (monotonic, wall-clock) correspondence pair, emitted periodically (see index.ts) so a
// workstation-side QA bookmark ('session-telemetry mark', a wall-clock reading with no live
// device bridge) can be mapped onto this session's own monotonic timeline. `timestamp` is the
// same performance.now() domain every other event uses; `wallClockUnixMs` is Date.now() read at
// that same instant - the one field in this protocol that legitimately is wall-clock.
export interface ClockSyncEvent {
  type: 'clock-sync';
  sequence: number;
  timestamp: number;
  wallClockUnixMs: number;
}

export type SessionTelemetryEvent =
  | RemoteInputEvent
  | FocusEvent
  | InteractionMarkerEvent
  | ReduxDispatchEvent
  | NetworkEvent
  | JsStallEvent
  | ReactCommitEvent
  | FrameTimingEvent
  | ClockSyncEvent;

// react-native-tvos's TVEventHandler still emits 'focus'/'blur' on the old architecture, but
// its own types document them as deprecated and not emitted under Fabric (New Architecture,
// which this library targets). Focus is recorded explicitly via recordFocus() from each
// component's onFocus prop instead; anything else TVEventHandler reports is treated as
// remote-control input.
const NON_REMOTE_INPUT_EVENT_TYPES: ReadonlySet<string> = new Set(['focus', 'blur']);

export function isRemoteInputEventType(eventType: string): boolean {
  return !NON_REMOTE_INPUT_EVENT_TYPES.has(eventType);
}
