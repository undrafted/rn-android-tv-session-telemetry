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

export type SessionTelemetryEvent =
  RemoteInputEvent | FocusEvent | InteractionMarkerEvent | ReduxDispatchEvent;

// react-native-tvos's TVEventHandler still emits 'focus'/'blur' on the old architecture, but
// its own types document them as deprecated and not emitted under Fabric (New Architecture,
// which is a hard V1 requirement — see plan.md section 3). Focus is recorded explicitly via
// recordFocus() from each component's onFocus prop instead; anything else TVEventHandler
// reports is treated as remote-control input.
const NON_REMOTE_INPUT_EVENT_TYPES: ReadonlySet<string> = new Set(['focus', 'blur']);

export function isRemoteInputEventType(eventType: string): boolean {
  return !NON_REMOTE_INPUT_EVENT_TYPES.has(eventType);
}
