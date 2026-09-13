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
  // React's monotonic render-start and commit timestamps, not callback observation time.
  // The elapsed interval can contain yields; actualDurationMs remains render work only.
  renderStartMs?: number;
  commitTimeMs?: number;
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

// A best-effort confirmation that a focus change's visual result was scheduled to paint - see
// FocusableView.tsx, which emits this from a requestAnimationFrame callback right after
// recordFocus. Not a guarantee the pixels were actually presented on screen, only that a render
// was queued for the next frame - treat the latency it yields as approximate, not frame-accurate.
export interface VisibleUpdateEvent {
  type: 'visible-update';
  sequence: number;
  timestamp: number;
  targetId: string;
}

// Device/build/session context, emitted once from SessionTelemetry.install() (see index.ts).
// deviceModel/osVersion come from React Native's own Platform module; appVersion/buildType are
// null unless the host app supplies them via InstallOptions - this library has no way to know
// its host's own version or build flavor on its own.
export interface SessionMetadataEvent {
  type: 'session-metadata';
  sequence: number;
  timestamp: number;
  deviceModel: string;
  osVersion: string;
  appVersion: string | null;
  buildType: string | null;
}

// One invocation of an application-opted-in selector, from telemetrySelector
// (packages/redux/src/index.ts) - a transparent passthrough wrapper, so this never affects the
// selector's own memoization semantics. inputsChanged compares this call's arguments against the
// previous call's by reference (Object.is per position); resultChanged compares the returned
// reference the same way. inputsChanged === false && resultChanged === true is the interesting
// case: the selector was called with the exact same arguments as last time but returned a
// different object/array reference anyway - a broken/unstable selector, not a real state change.
// Metadata only: never the selector's arguments or its result value.
export interface SelectorEvent {
  type: 'selector';
  sequence: number;
  timestamp: number;
  selectorId: string;
  durationMs: number;
  inputsChanged: boolean;
  resultChanged: boolean;
}

// Opens an explicit CPU/memory sampling window (see SessionTelemetry.startResourceSampling in
// index.ts). Unlike network/JS-stall/React-commit capture, resource sampling is never started
// automatically from install() - its overhead is high enough that the application must open a
// window itself, the same opt-in posture as Redux middleware. intervalMs is this window's
// configured sampling cadence, disclosed so a report consumer can judge sample coarseness.
export interface ResourceSamplingStartedEvent {
  type: 'resource-sampling-started';
  sequence: number;
  timestamp: number;
  intervalMs: number;
}

// One periodic sample taken while a resource-sampling window is open. cpuUtilizationPercent is
// the process's CPU time delta since the previous sample divided by the elapsed wall-clock
// delta, as a percentage of one core - it can exceed 100 on a multi-core device actively using
// more than one thread, and must not be presented as method-level attribution. nativeHeapKb/
// javaHeapKb are the managed/native breakdown Debug.getNativeHeapAllocatedSize()/Runtime heap
// usage give without the more expensive ActivityManager.getProcessMemoryInfo cross-process query.
export interface ResourceSampleEvent {
  type: 'resource-sample';
  sequence: number;
  timestamp: number;
  cpuUtilizationPercent: number;
  nativeHeapKb: number;
  javaHeapKb: number;
}

// Closes a resource-sampling window opened by a matching ResourceSamplingStartedEvent.
export interface ResourceSamplingStoppedEvent {
  type: 'resource-sampling-stopped';
  sequence: number;
  timestamp: number;
}

// An app-wide foreground/background transition, captured automatically once install() runs (see
// lifecycle.ts) - no application call needed, the same self-contained tier as network and
// JS-stall capture (unlike frame timing/focus, which the app still starts explicitly). Reflects
// Application.ActivityLifecycleCallbacks' started/stopped counts, not a single Activity's own
// state, so this stays correct even if the host app has more than one Activity.
export interface LifecycleEvent {
  type: 'lifecycle';
  sequence: number;
  timestamp: number;
  state: 'foreground' | 'background';
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
  | ClockSyncEvent
  | VisibleUpdateEvent
  | SessionMetadataEvent
  | SelectorEvent
  | ResourceSamplingStartedEvent
  | ResourceSampleEvent
  | ResourceSamplingStoppedEvent
  | LifecycleEvent;

// react-native-tvos's TVEventHandler still emits 'focus'/'blur' on the old architecture, but
// its own types document them as deprecated and not emitted under Fabric (New Architecture,
// which this library targets). Focus is recorded explicitly via recordFocus() from each
// component's onFocus prop instead; anything else TVEventHandler reports is treated as
// remote-control input.
const NON_REMOTE_INPUT_EVENT_TYPES: ReadonlySet<string> = new Set(['focus', 'blur']);

export function isRemoteInputEventType(eventType: string): boolean {
  return !NON_REMOTE_INPUT_EVENT_TYPES.has(eventType);
}
