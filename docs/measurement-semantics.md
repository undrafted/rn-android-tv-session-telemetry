### Measurement semantics

See [the signals diagram](signals.md) for the whole capture-to-report pipeline at a glance
before reading the per-signal detail below.

### Signal capture

All signals below are exercised by the TV fixture.

| Signal             | Source and limits                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Remote input       | `TVEventHandler.addListener`, filtered by `isRemoteInputEventType`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Focus              | `GlobalFocusModule.kt` tracks app-wide focus by RN `nativeID`. No per-view handler needed.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                              |
| Visible update     | Posted callback after focus, on the view's UI-thread queue — an approximation, not a frame-presentation timestamp.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Interaction marker | Explicit `SessionTelemetry.mark()` calls in app code. Distinct from the CLI's `session-telemetry mark` command, which a person types on the workstation, not something the app runs — same idea, two different sources.                                                                                                                                                                                                                                                                                                                                                                                 |
| Redux dispatch     | `createTelemetryMiddleware` from `packages/redux`. Action type and duration only — `includePayloads: false` is a literal type on the options, not a default; the payload can't be turned on.                                                                                                                                                                                                                                                                                                                                                                                                            |
| Redux selector     | `telemetrySelector()` calls once, returns the result, and compares args/result to the last call via `Object.is`. First call: both change flags `true`. Metadata only — never the selector's actual arguments or result value.                                                                                                                                                                                                                                                                                                                                                                           |
| Network request    | `install()` observes RN fetch/XHR traffic (not independent/native transports). One terminal event per request; duration ends at completion. Sizes are estimates or `null`. Credentials/fragments/query params are stripped except allowlisted ones. See the [capture boundary](../diagrams/network.svg).                                                                                                                                                                                                                                                                                                |
| JS stall           | `install()` starts one timer (reinstall replaces it, `stop()` clears it); defaults 50ms interval/threshold. Checks lateness against a scheduled deadline, not CPU usage — a busy JS thread can't run the check on time, so overshoot is a proxy for blockage, never its cause (no stack traces). Interval clamped to 16–2,147,483,647ms: 16ms is one frame at 60fps, below which polling costs more than it detects; 2,147,483,647 (2³¹−1) is `setTimeout`'s own 32-bit-integer delay limit — past it the timer fires almost immediately instead of waiting, so the clamp prevents that silent misfire. |
| React commit       | Root `Profiler` callback — see [React commit detail](#react-commit-detail) below for fields and setup.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Frame timing       | `FrameTimingModule.kt` reads `FrameMetrics.TOTAL_DURATION`; only frames over `thresholdMs` (32ms default) reach JS.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                     |
| Clock sync         | First event after `install()`, then every 30s of pushed-event activity.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Session metadata   | Emitted at `install()`. Device model/OS from RN `Platform`; app version/build type from `InstallOptions`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Resource sample    | Off by default — `startResourceSampling(options?)`/`stopResourceSampling()` open/close an explicit window (overhead-driven opt-in, same posture as Redux middleware). `ResourceSamplingModule.kt` samples on a `HandlerThread` at the given interval (default 500ms). CPU: `Process.getElapsedCpuTime()` delta ÷ wall-clock delta, as % of one core (can exceed 100 on multi-core; never method-level attribution). Memory: native heap + JVM used heap, not the costlier `ActivityManager.getProcessMemoryInfo`. No events outside an open window.                                                     |
| Lifecycle          | Automatic at `install()`, the same self-contained tier as network/JS-stall (unlike frame timing/focus, which the app still starts explicitly). `LifecycleModule.kt` tracks `Application.ActivityLifecycleCallbacks`' started/stopped counts, not a single Activity's own state — correct even with more than one Activity. `foreground` fires when the count goes 0→1, `background` when it drops back to 0.                                                                                                                                                                                            |

### Analysis

"Evidence only" means a signal can appear in a finding's event range without a detector using it.
Interaction marker and lifecycle stay evidence-only because neither carries a value the library
can threshold: a marker's `name` is free-form app text with no fixed vocabulary, and a lifecycle
transition is a state change, not a duration or count. Both exist for a human (or an app-specific
detector built on top) reading the report, not automated pattern-matching.

| Signal             | Consumer                                                                                                                                                          |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Remote input       | Starts an `InteractionWindow`.                                                                                                                                    |
| Focus              | `high-latency-focus-change`                                                                                                                                       |
| Visible update     | `high-latency-visible-update`; ends the interaction window scan.                                                                                                  |
| Interaction marker | Evidence only                                                                                                                                                     |
| Redux dispatch     | `repeated-redux-dispatch`                                                                                                                                         |
| Redux selector     | `repeated-selector-recomputation`, `unstable-selector-reference`, session-wide `Report.selectorStats`                                                             |
| Network request    | `repeated-network-request`, `network-completion-followed-by-commit`                                                                                               |
| JS stall           | `js-stall-during-interaction`                                                                                                                                     |
| React commit       | `react-commit-overlapping-delayed-frame`, `network-completion-followed-by-commit`, `excessive-commits-during-rapid-focus-movement`, `unstable-selector-reference` |
| Frame timing       | `react-commit-overlapping-delayed-frame`                                                                                                                          |
| Clock sync         | Bookmark mapping, `Report.clockUncertaintyMs`                                                                                                                     |
| Session metadata   | `Report.deviceMetadata` (first matching event)                                                                                                                    |
| Resource sample    | `high-cpu-sustained-during-resource-sampling`, `memory-grew-across-repeated-resource-sampling-windows`, `Report.resourceSamplingWindows`                          |
| Lifecycle          | Evidence only                                                                                                                                                     |

Detector thresholds are provisional, pending calibration on TV hardware.

| Detector                                                | Trigger                                                                      | Severity                        |
| ------------------------------------------------------- | ---------------------------------------------------------------------------- | ------------------------------- |
| `high-latency-focus-change`                             | Input-to-focus latency                                                       | Warning ≥100ms; critical ≥300ms |
| `high-latency-visible-update`                           | Input-to-visible-update latency                                              | Warning ≥100ms; critical ≥300ms |
| `repeated-redux-dispatch`                               | Same action type ≥2× in one interaction                                      | Warning                         |
| `js-stall-during-interaction`                           | Any captured stall                                                           | Warning; critical ≥150ms        |
| `repeated-network-request`                              | ≥2 equivalent requests in one interaction; streams ≥2000ms excluded          | Warning                         |
| `react-commit-overlapping-delayed-frame`                | Render-to-commit interval overlaps an estimated delayed-frame span           | Warning                         |
| `network-completion-followed-by-commit`                 | A commit is the next event after a network response                          | Warning                         |
| `excessive-commits-during-rapid-focus-movement`         | ≥3 commits across a focus burst, gaps ≤150ms                                 | Warning                         |
| `repeated-selector-recomputation`                       | Same selector called ≥2× with unchanged inputs in one interaction            | Warning                         |
| `unstable-selector-reference`                           | New result reference with unchanged inputs, window has ≥2 React commits      | Warning                         |
| `high-cpu-sustained-during-resource-sampling`           | Average CPU across ≥3 samples in one resource-sampling window                | Warning ≥70%; critical ≥90%     |
| `memory-grew-across-repeated-resource-sampling-windows` | Combined native+JS heap strictly higher every window across ≥3, total ≥512KB | Warning                         |

### React commit detail

| Field                                             | Meaning                                                                             |
| ------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `actualDurationMs`                                | Render work for the committed subtree.                                              |
| `baseDurationMs`                                  | React's estimated cost without memoization.                                         |
| `renderStartMs` / `commitTimeMs`                  | React's own render-start and commit timestamps.                                     |
| `timestamp`                                       | When the callback observed the commit (not the same instant as `commitTimeMs`).     |
| `reactCommitCapture: observed`                    | At least one commit recorded; capture may still be incomplete.                      |
| `reactCommitCapture: unavailable-or-not-observed` | No commit recorded — could be unsupported renderer, unwrapped root, or no activity. |

Render-to-commit elapsed time can include pauses; it's separate from render work, commit-phase
duration, and frame presentation. Overlap analysis needs valid renderer timestamps and never
establishes causation.

Setup: `withTelemetryRoot(App, appName)` wraps the app in a library-owned `Profiler` in profiling
builds; `withReactProfiling` selects the Fabric profiling renderer in Metro (the production
renderer emits nothing). `stop()` disables recording but keeps the root mounted, preserving
state — reinstall resumes through the same wrapper. Verified scope: Android TV, Fabric/Hermes,
RN TV 0.87.1-0, React 19.2.3 — root commits only, no component/props/state, profiling adds
overhead.

Verified on a real TV capture: seven commits preserved `renderStartMs ≤ commitTimeMs ≤ timestamp`;
the mount callback arrived ~21ms after commit.

### Report output

`reactSummary.byProfiler`/`reactSummary.interactions` give commit count, valid-duration count,
total render work, and longest render, grouped by profiler ID and by interaction window
respectively (nested profiler totals can overlap; negative/non-finite durations are excluded
from totals). HTML shows the first 200 interaction summaries; JSON pages hold the rest.

Large sessions export as a paged bundle, not one flat JSON file: `<capture>.json` is an index
(session totals, capture availability, counts, relative page paths); event/finding/interaction/
profiler/selector/bookmark pages hold ≤1,000 records each and live alongside it — keep both when
sharing a report. Each event exports once; a finding references its inclusive
`sequenceStart`–`sequenceEnd`, and page bounds locate the actual evidence. Pages write before the
index publishes, so a failed export leaves the prior index usable. HTML previews cap at 100
findings (20 evidence events each), 200 interactions/profilers/selectors/bookmarks, and 5,000
timeline events — the complete data is always in the JSON pages.
`Report::to_json()` (in-process, not the CLI's export) stays a single complete document instead,
for small consumers that don't need paging.

Synthetic 30-minute worst case (18,000 events, 1,800 deliberately overlapping findings — a
duplication stress test, not a typical distribution): 2.87MB total output versus 2.78GB estimated
unpaged. ~88ms construction/export, 14.5MB peak RSS (debug build, excludes decoding/detectors).
Reproduce: `cargo run -p session-telemetry-report --example report_size -- /tmp/rnst-report-size`.

### Session and clock semantics

| Topic              | Behavior                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| ------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Clocks             | Monotonic only: `performance.now()` in JS, Android's native clock for frame timing. Timestamps/durations are milliseconds; sequence numbers are counters, one per JS process — `install()`/`stop()` don't reset it, restarting the app does. Chunk decoding and interaction windows sort and index on sequence, not timestamp — evidence and `sequenceStart`/`sequenceEnd` ranges (see [Report output](#report-output)) are always sequence-based. |
| Session boundary   | App calls and CLI start/stop broadcasts open or seal the native session; starting one seals any open one first. `startNativeSession()` runs before metadata/clock-sync events emit — the native writer ignores pushes while no session is open.                                                                                                                                                                                                    |
| Clock mapping      | `ClockSyncEvent.wallClockUnixMs` maps workstation bookmarks (from `session-telemetry mark`, the CLI command — not app code) onto the session timeline. `ClockMap` fits offset+scale by least squares; `uncertainty_ms` is the largest fitted residual, not a guaranteed bound (`0.0` with one sample, `null` with none).                                                                                                                           |
| Event loss         | `Chunk.manifest.lossCount` is always `0` — gap detection isn't implemented, so this doesn't establish nothing was lost.                                                                                                                                                                                                                                                                                                                            |
| Storage budget     | Chunks rotate at 256KB/60s; session budget 200MB. Reaching it stops writes and logs once; sealed chunks stay available. Defaults still need device validation.                                                                                                                                                                                                                                                                                     |
| Format             | Recordings/reports are unversioned during development. Update the collector and CLI together; older captures aren't guaranteed compatible.                                                                                                                                                                                                                                                                                                         |
| Overhead benchmark | Ten alternating active/stopped cases: call cost (mark/Redux/stall/React), network, idle/render timing, and a resource-sampling window at two intervals. Marker median 0.039ms/call; resource-sampling window +3.6ms (500ms interval) / +2.4ms (16ms interval) scheduling lateness over 2s. Full table: [benchmark results](../apps/tv-fixture/README.md#benchmark-results--2026-09-13).                                                            |
