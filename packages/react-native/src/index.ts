import { startStallMonitor, stopStallMonitor, type StallMonitorOptions } from './stall.js';
import { startNetworkCapture, type NormalizeUrlOptions } from './network.js';
import { Platform, TVEventHandler, type EventSubscription, type HWEvent } from 'react-native';
import { createSequenceCounter, monotonicNowMs } from './clock.js';
import { isRemoteInputEventType, type SessionTelemetryEvent } from './events.js';
import {
  finishNativeSession,
  onNativeSessionOpened,
  startNativeSession,
  transferEventToNative,
} from './nativeTransfer.js';
import { startNativeResourceSampling, stopNativeResourceSampling } from './resourceSampling.js';
import { startLifecycleMonitor } from './lifecycle.js';

export type {
  SessionTelemetryEvent,
  RemoteInputEvent,
  FocusEvent,
  InteractionMarkerEvent,
  ReduxDispatchEvent,
  NetworkEvent,
  JsStallEvent,
  ReactCommitEvent,
  FrameTimingEvent,
  VisibleUpdateEvent,
  SessionMetadataEvent,
  SelectorEvent,
  ResourceSamplingStartedEvent,
  ResourceSampleEvent,
  ResourceSamplingStoppedEvent,
  LifecycleEvent,
} from './events.js';
export { startGlobalFocusMonitor } from './globalFocus.js';
export { startLifecycleMonitor } from './lifecycle.js';
export { normalizeUrl, createInstrumentedFetch, type NormalizeUrlOptions } from './network.js';
export { startStallMonitor, type StallMonitorOptions } from './stall.js';
export { onProfilerRender, withTelemetryRoot } from './profiler.js';
export { startFrameTimingMonitor, type FrameTimingMonitorOptions } from './frameTiming.js';

export interface InstallOptions {
  // Automatic bounded event-loop lateness sampling; no separate startup helper required.
  stall?: StallMonitorOptions;
  // Automatic capture of RN fetch and XHR-backed requests; no bodies or headers are stored.
  network?: NormalizeUrlOptions;
  // Once the buffer reaches this size, the oldest event is dropped for each new one recorded
  // (a sliding window), and droppedEventCount increments — bounded memory over a multi-hour QA
  // capture instead of an unbounded array. Clamped to >= 1.
  maxBufferedEvents?: number;
  // This library has no way to know its host app's own version or build flavor on its own -
  // these are carried straight into the session's one-time SessionMetadataEvent when supplied,
  // and left null otherwise (not guessed).
  appVersion?: string;
  buildType?: string;
}

export interface ResourceSamplingOptions {
  // How often to sample process CPU/memory while this window is open, in ms. Defaults to 500
  // (the plan's "targeted" profile) - a QA capture that wants the coarser "1-5s" profile passes
  // a larger value explicitly. Clamped to >= 16.
  intervalMs?: number;
}

export interface SessionTelemetryApi {
  install(options?: InstallOptions): void;
  stop(): void;
  mark(name: string): void;
  recordFocus(targetId: string): void;
  recordVisibleUpdate(targetId: string): void;
  recordDispatch(actionType: string, durationMs: number): void;
  // Opens an explicit CPU/memory sampling window - unlike every other automatic signal in this
  // API, this is never started from install() on its own. Its overhead is high enough that it
  // stays the application's own decision to make, and when: at startup only, or scoped tightly
  // around one scenario (e.g. a stream/playback request), the same opt-in posture as wiring in
  // Redux middleware. A no-op if a window is already open.
  startResourceSampling(options?: ResourceSamplingOptions): void;
  // Closes the window opened by startResourceSampling. A no-op if none is open.
  stopResourceSampling(): void;
  recordResourceSample(
    cpuUtilizationPercent: number,
    nativeHeapKb: number,
    javaHeapKb: number,
  ): void;
  recordNetworkRequest(
    method: string,
    url: string,
    status: number,
    durationMs: number,
    requestBytes: number | null,
    responseBytes: number | null,
  ): void;
  recordJsStall(durationMs: number): void;
  recordFrameTiming(durationMs: number): void;
  recordReactCommit(
    profilerId: string,
    phase: 'mount' | 'update' | 'nested-update',
    actualDurationMs: number,
    baseDurationMs: number,
    renderStartMs?: number,
    commitTimeMs?: number,
  ): void;
  recordSelector(
    selectorId: string,
    durationMs: number,
    inputsChanged: boolean,
    resultChanged: boolean,
  ): void;
  recordLifecycleTransition(state: 'foreground' | 'background'): void;
  // Temporary: exposes the in-memory buffer until a native chunk writer exists. Not part of
  // the stable V1 API surface.
  getBufferedEvents(): readonly SessionTelemetryEvent[];
  // How many events the sliding window has evicted this session — the "event loss ... visible
  // in the report" disclosure success criterion #9 requires, at least on the JS side of it.
  getDroppedEventCount(): number;
}

const DEFAULT_MAX_BUFFERED_EVENTS = 10_000;

// How often a clock-sync sample piggybacks on an ordinary event push (see maybeEmitClockSync
// below), not a timer - sample density then scales with actual session activity instead of
// costing anything when idle. 30s keeps drift-fitting (ClockMap fits offset *and* scale, see
// session-telemetry-analysis) meaningful across a long QA session without spamming samples.
const CLOCK_SYNC_INTERVAL_MS = 30_000;

const nextSequence = createSequenceCounter();

let stopNetworkCapture: (() => void) | undefined;
let stopLifecycleMonitor: (() => void) | undefined;
let subscription: EventSubscription | undefined;
let nativeSessionOpenedUnsubscribe: (() => void) | undefined;
let previousFocusTarget: string | null = null;
let buffer: SessionTelemetryEvent[] = [];
let maxBufferedEvents = DEFAULT_MAX_BUFFERED_EVENTS;
let droppedEventCount = 0;
let lastClockSyncAt: number | null = null;
let resourceSamplingActive = false;
// Recording methods (mark/recordFocus/handleHardwareEvent) are real no-ops until install()
// runs. App code calls mark()/recordFocus() unconditionally from UI handlers rather than
// re-checking the build flag at every call site, so this is what actually keeps a disabled
// build from silently growing the buffer forever.
let installed = false;

function pushToBufferAndNative(event: SessionTelemetryEvent): void {
  if (buffer.length >= maxBufferedEvents) {
    buffer.shift();
    droppedEventCount += 1;
  }
  buffer.push(event);
  // Transferred unconditionally, independent of the sliding window above - the whole point of
  // the native/on-device writer is durability past what the bounded JS buffer keeps in memory.
  transferEventToNative(event);
}

function emitClockSync(): void {
  const now = monotonicNowMs();
  lastClockSyncAt = now;
  pushToBufferAndNative({
    type: 'clock-sync',
    sequence: nextSequence(),
    timestamp: now,
    wallClockUnixMs: Date.now(),
  });
}

// Piggybacks a clock-sync sample on whichever real event happens to be firing, rather than a
// setInterval - ties sample density to actual session activity (a QA engineer generating a
// bookmark-worthy moment is, by definition, usually also generating other events) with no
// battery cost when the session is idle. Skips emitting one for the very first push after
// install() - lastClockSyncAt starts null specifically so that first call always samples,
// giving every session a baseline sample even if it's short.
//
// Must run, as a statement, before its caller computes `sequence: nextSequence()` for its own
// event - not folded into a shared pushEvent(event) wrapper, because a function's arguments
// (including that nextSequence() call) evaluate before the function body runs. A wrapper would
// let the caller's sequence number get allocated before this function's own nextSequence() call
// even though this function's sample is pushed to the buffer first, producing a buffer whose
// insertion order silently disagrees with its own sequence numbers.
function maybeEmitClockSync(): void {
  const now = monotonicNowMs();
  if (lastClockSyncAt !== null && now - lastClockSyncAt < CLOCK_SYNC_INTERVAL_MS) {
    return;
  }
  emitClockSync();
}

// Emitted once per install() - deviceModel/osVersion come from React Native's own Platform
// module (already available on Android with no new native code); appVersion/buildType come
// from the host app's own InstallOptions, since this library can't know them itself.
function emitSessionMetadata(options?: InstallOptions): void {
  maybeEmitClockSync();
  // Platform.constants' shape is a per-OS union (Model/Release only exist on the Android
  // variant) - this library targets Android TV specifically (see this repo's own conventions),
  // but narrows explicitly rather than assuming, so a non-Android host reports 'unknown' instead
  // of a wrong guess.
  const deviceModel = Platform.OS === 'android' ? Platform.constants.Model : 'unknown';
  const osVersion = Platform.OS === 'android' ? String(Platform.Version) : 'unknown';
  pushToBufferAndNative({
    type: 'session-metadata',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    deviceModel,
    osVersion,
    appVersion: options?.appVersion ?? null,
    buildType: options?.buildType ?? null,
  });
}

function handleHardwareEvent(event: HWEvent): void {
  if (!installed || !isRemoteInputEventType(event.eventType)) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'remote-input',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    key: event.eventType,
  });
}

function install(options?: InstallOptions): void {
  stopStallMonitor();
  stopNetworkCapture?.();
  stopLifecycleMonitor?.();
  subscription?.remove();
  nativeSessionOpenedUnsubscribe?.();
  // A window left open from a previous install() must not silently carry into this one - unlike
  // network/stall capture, this never restarts itself; the application must call
  // startResourceSampling() again if it still wants one.
  if (resourceSamplingActive) {
    stopNativeResourceSampling();
    resourceSamplingActive = false;
  }
  buffer = [];
  droppedEventCount = 0;
  maxBufferedEvents = Math.max(1, options?.maxBufferedEvents ?? DEFAULT_MAX_BUFFERED_EVENTS);
  previousFocusTarget = null;
  installed = true;
  lastClockSyncAt = null;
  subscription = TVEventHandler.addListener(handleHardwareEvent);
  // A later `session-telemetry record` can open a *new* native session on its own (ADB
  // broadcast, independent of this call - see this library's bidirectional design), which the
  // baseline/periodic sampling above has no way to know about on its own. This forces a fresh
  // sample the moment native confirms that happened, so a short broadcast-triggered session
  // isn't left with zero clock-sync coverage for its entire lifetime - confirmed as a real gap
  // against a live device, not a hypothetical. Symmetric with the TVEventHandler subscription
  // above: re-subscribed on every install(), torn down on stop().
  nativeSessionOpenedUnsubscribe = onNativeSessionOpened(() => {
    emitClockSync();
  });
  // This is the app's own enable trigger - see nativeTransfer.ts for why this must actually
  // start durable on-device recording, not just the JS-side buffer/subscriptions above. Must
  // run before emitSessionMetadata below: the native pushEvent bridge call is a no-op until a
  // session is open (SessionWriterModule.pushEvent's handle==0 guard), and calling start()
  // first guarantees native processes it before the pushEvent call that follows in the same JS
  // tick (bridge calls to one module are dispatched in the order JS made them) - reversed, the
  // baseline clock-sync and session-metadata events would reach native before any session
  // existed to hold them and be silently dropped every time, confirmed against a real device
  // trace, not a hypothetical.
  startNativeSession();
  emitSessionMetadata(options);
  stopNetworkCapture = startNetworkCapture(recordNetworkRequest, options?.network);
  startStallMonitor(options?.stall);
  stopLifecycleMonitor = startLifecycleMonitor();
}

function stop(): void {
  installed = false;
  stopStallMonitor();
  stopNetworkCapture?.();
  stopNetworkCapture = undefined;
  stopLifecycleMonitor?.();
  stopLifecycleMonitor = undefined;
  subscription?.remove();
  subscription = undefined;
  nativeSessionOpenedUnsubscribe?.();
  nativeSessionOpenedUnsubscribe = undefined;
  // Closes any window the application left open rather than leaking the native sampler past the
  // session's own end - emits the same resource-sampling-stopped event a normal
  // stopResourceSampling() call would (a no-op if none was open), so the report can't mistake
  // this for a window that just never closed.
  stopResourceSampling();
  finishNativeSession();
}

function mark(name: string): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
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
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'focus',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    targetId,
    previousTargetId: previousFocusTarget,
  });
  previousFocusTarget = targetId;
}

function recordVisibleUpdate(targetId: string): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'visible-update',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    targetId,
  });
}

function recordDispatch(actionType: string, durationMs: number): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'redux-dispatch',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    actionType,
    durationMs,
  });
}

function recordNetworkRequest(
  method: string,
  url: string,
  status: number,
  durationMs: number,
  requestBytes: number | null,
  responseBytes: number | null,
): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'network',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    method,
    url,
    status,
    durationMs,
    requestBytes,
    responseBytes,
  });
}

function recordJsStall(durationMs: number): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'js-stall',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    durationMs,
  });
}

function recordFrameTiming(durationMs: number): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'frame-timing',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    durationMs,
  });
}

function recordReactCommit(
  profilerId: string,
  phase: 'mount' | 'update' | 'nested-update',
  actualDurationMs: number,
  baseDurationMs: number,
  renderStartMs?: number,
  commitTimeMs?: number,
): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'react-commit',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    profilerId,
    phase,
    actualDurationMs,
    baseDurationMs,
    ...(renderStartMs !== undefined && Number.isFinite(renderStartMs) ? { renderStartMs } : {}),
    ...(commitTimeMs !== undefined && Number.isFinite(commitTimeMs) ? { commitTimeMs } : {}),
  });
}

function recordSelector(
  selectorId: string,
  durationMs: number,
  inputsChanged: boolean,
  resultChanged: boolean,
): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'selector',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    selectorId,
    durationMs,
    inputsChanged,
    resultChanged,
  });
}

function recordLifecycleTransition(state: 'foreground' | 'background'): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'lifecycle',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    state,
  });
}

// A short-interval default suited to a targeted capture - a QA capture wanting coarser, lower-
// overhead sampling over a longer session passes a larger intervalMs explicitly.
const DEFAULT_RESOURCE_SAMPLING_INTERVAL_MS = 500;

function startResourceSampling(options?: ResourceSamplingOptions): void {
  if (!installed || resourceSamplingActive) {
    return;
  }
  const intervalMs = Math.max(16, options?.intervalMs ?? DEFAULT_RESOURCE_SAMPLING_INTERVAL_MS);
  resourceSamplingActive = true;
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'resource-sampling-started',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    intervalMs,
  });
  startNativeResourceSampling(intervalMs, (sample) => {
    recordResourceSample(sample.cpuUtilizationPercent, sample.nativeHeapKb, sample.javaHeapKb);
  });
}

function stopResourceSampling(): void {
  if (!resourceSamplingActive) {
    return;
  }
  resourceSamplingActive = false;
  stopNativeResourceSampling();
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'resource-sampling-stopped',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
  });
}

function recordResourceSample(
  cpuUtilizationPercent: number,
  nativeHeapKb: number,
  javaHeapKb: number,
): void {
  if (!installed) {
    return;
  }
  maybeEmitClockSync();
  pushToBufferAndNative({
    type: 'resource-sample',
    sequence: nextSequence(),
    timestamp: monotonicNowMs(),
    cpuUtilizationPercent,
    nativeHeapKb,
    javaHeapKb,
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
  recordVisibleUpdate,
  recordDispatch,
  recordNetworkRequest,
  recordJsStall,
  recordFrameTiming,
  recordReactCommit,
  recordSelector,
  recordLifecycleTransition,
  startResourceSampling,
  stopResourceSampling,
  recordResourceSample,
  getBufferedEvents,
  getDroppedEventCount,
};
