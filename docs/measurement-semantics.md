### Measurement semantics

See [the signals diagram](signals.md) for the whole capture-to-report pipeline at a glance
before reading the per-signal detail below.

| Topic               | Behavior                                                                                                                                                                                                                                                                                                                                                                                |
| ------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Clocks              | Monotonic only: `performance.now()` in JS, Android's native clock for frame timing. `ClockSyncEvent.wallClockUnixMs` maps workstation bookmarks (from `session-telemetry mark`, the CLI command — not app code) onto the session timeline.                                                                                                                                              |
| Units               | Timestamps/durations in milliseconds. Sequence numbers are counters.                                                                                                                                                                                                                                                                                                                    |
| Sequence numbers    | One counter per JS process. `install()`/`stop()` don't reset it; restarting the app does.                                                                                                                                                                                                                                                                                               |
| Session boundary    | App calls and CLI start/stop broadcasts open or seal the native session. Starting a session seals any open one first.                                                                                                                                                                                                                                                                   |
| Startup ordering    | `startNativeSession()` runs before metadata/clock-sync events emit. The native writer ignores pushes while no session is open.                                                                                                                                                                                                                                                          |
| Overhead benchmark  | Ten alternating active/stopped cases: call cost (mark/Redux/stall/React), network, idle/render timing, and a resource-sampling window at two intervals. Marker median 0.039ms/call; resource-sampling window +3.6ms (500ms interval) / +2.4ms (16ms interval) scheduling lateness over 2s. Full table: [benchmark results](../apps/tv-fixture/README.md#benchmark-results--2026-09-13). |
| Event loss          | `Chunk.manifest.lossCount` is always `0` — gap detection isn't implemented, so this doesn't establish nothing was lost.                                                                                                                                                                                                                                                                 |
| Clock mapping       | `ClockMap` fits offset+scale by least squares. `uncertainty_ms` is the largest fitted residual, not a guaranteed bound (`0.0` with one sample). `null` when no samples exist.                                                                                                                                                                                                           |
| Storage budget      | Chunks rotate at 256KB/60s; session budget 200MB. Reaching it stops writes and logs once; sealed chunks stay available. Defaults still need device validation.                                                                                                                                                                                                                          |
| Native registration | `RnSessionTelemetryPackage` registers every native module via autolinking. Add new modules to its `createNativeModules()` list.                                                                                                                                                                                                                                                         |
| Format              | Recordings/reports are unversioned during development. Update the collector and CLI together; older captures aren't guaranteed compatible.                                                                                                                                                                                                                                              |

### React timing

| Field                                             | Meaning                                                                             |
| ------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `actualDurationMs`                                | Render work for the committed subtree.                                              |
| `baseDurationMs`                                  | React's estimated cost without memoization.                                         |
| `renderStartMs`                                   | When React started rendering.                                                       |
| `commitTimeMs`                                    | React's commit timestamp.                                                           |
| `timestamp`                                       | When the callback observed the commit.                                              |
| `reactCommitCapture: observed`                    | At least one commit recorded; capture may still be incomplete.                      |
| `reactCommitCapture: unavailable-or-not-observed` | No commit recorded — could be unsupported renderer, unwrapped root, or no activity. |

Render-to-commit elapsed time can include pauses; it's separate from render work, commit-phase
duration, and frame presentation. Overlap analysis needs valid renderer timestamps and never
establishes causation.

| Setup               | Behavior                                                                                                                |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| Root registration   | `withTelemetryRoot(App, appName)` wraps the app in a library-owned `Profiler` in profiling builds.                      |
| Renderer            | `withReactProfiling` selects the Fabric profiling renderer in Metro; the production renderer emits nothing.             |
| Recording lifecycle | `stop()` disables recording but keeps the root mounted, preserving state; reinstall resumes through it.                 |
| Scope               | Android TV, Fabric/Hermes, RN TV 0.87.1-0, React 19.2.3. Root commits only, no component/props/state. Overhead applies. |

Verified on a real TV capture: seven commits preserved `renderStartMs ≤ commitTimeMs ≤ timestamp`;
the mount callback arrived ~21ms after commit.

### Signal capture

All signals below are exercised by the TV fixture.

| Signal             | Source and limits                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                   |
| ------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Remote input       | `TVEventHandler.addListener`, filtered by `isRemoteInputEventType`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Focus              | `GlobalFocusModule.kt` tracks app-wide focus by RN `nativeID`. No per-view handler needed.                                                                                                                                                                                                                                                                                                                                                                                                                                                          |
| Visible update     | Posted callback after focus, on the view's UI-thread queue — an approximation, not a frame-presentation timestamp.                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Interaction marker | Explicit `SessionTelemetry.mark()` calls in app code. Distinct from the CLI's `session-telemetry mark` command below, which a person types on the workstation, not something the app runs — same idea, two different sources.                                                                                                                                                                                                                                                                                                                       |
| Redux dispatch     | `createTelemetryMiddleware` from `packages/redux`.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Redux selector     | `telemetrySelector()` calls once, returns the result, and compares args/result to the last call via `Object.is`. First call: both change flags `true`.                                                                                                                                                                                                                                                                                                                                                                                              |
| Network request    | `install()` observes RN fetch/XHR traffic (not independent/native transports). One terminal event per request; duration ends at completion. Sizes are estimates or `null`. Credentials/fragments/query params are stripped except allowlisted ones. See the [capture boundary](../diagrams/network.svg).                                                                                                                                                                                                                                            |
| JS stall           | `install()` starts one timer (reinstall replaces it, `stop()` clears it); defaults 50ms interval/threshold, clamped 16–2,147,483,647ms. Records overshoot only, no stack traces.                                                                                                                                                                                                                                                                                                                                                                    |
| React commit       | Root `Profiler` callback; see timing fields above.                                                                                                                                                                                                                                                                                                                                                                                                                                                                                                  |
| Frame timing       | `FrameTimingModule.kt` reads `FrameMetrics.TOTAL_DURATION`; only frames over `thresholdMs` (32ms default) reach JS.                                                                                                                                                                                                                                                                                                                                                                                                                                 |
| Clock sync         | First event after `install()`, then every 30s of pushed-event activity.                                                                                                                                                                                                                                                                                                                                                                                                                                                                             |
| Session metadata   | Emitted at `install()`. Device model/OS from RN `Platform`; app version/build type from `InstallOptions`.                                                                                                                                                                                                                                                                                                                                                                                                                                           |
| Resource sample    | Off by default — `startResourceSampling(options?)`/`stopResourceSampling()` open/close an explicit window (overhead-driven opt-in, same posture as Redux middleware). `ResourceSamplingModule.kt` samples on a `HandlerThread` at the given interval (default 500ms). CPU: `Process.getElapsedCpuTime()` delta ÷ wall-clock delta, as % of one core (can exceed 100 on multi-core; never method-level attribution). Memory: native heap + JVM used heap, not the costlier `ActivityManager.getProcessMemoryInfo`. No events outside an open window. |
| Lifecycle          | Automatic at `install()`, the same self-contained tier as network/JS-stall (unlike frame timing/focus, which the app still starts explicitly). `LifecycleModule.kt` tracks `Application.ActivityLifecycleCallbacks`' started/stopped counts, not a single Activity's own state — correct even with more than one Activity. `foreground` fires when the count goes 0→1, `background` when it drops back to 0.                                                                                                                                        |

### React summaries

| Output                      | Meaning                                                                                                                    |
| --------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `reactSummary.byProfiler`   | Commit count, valid-duration count, total render work, longest render — grouped by profiler ID. Nested totals can overlap. |
| `reactSummary.interactions` | Same metrics per input window, ending at the first visible update or next input. Association isn't attribution.            |
| Missing durations           | Negative/non-finite durations excluded from totals; totals are `null` when none are valid.                                 |
| HTML                        | First 200 interaction summaries shown; JSON pages hold the rest. Elapsed render-to-commit isn't summed.                    |

### Large reports

| Output                    | Contents                                                                                                                                                    |
| ------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `<capture>.json`          | Report index: session totals, capture availability, counts, relative page paths.                                                                            |
| `<capture>.json.pages-…/` | Event/finding/interaction/profiler/selector/bookmark pages, ≤1,000 records each. Keep with the index when sharing.                                          |
| Evidence                  | Each event exported once. A finding references its inclusive `sequenceStart`–`sequenceEnd`; page bounds locate it.                                          |
| HTML previews             | First 100 findings (20 evidence events each), 200 interactions/profilers/selectors/bookmarks, 5,000 timeline events — the rest is in the linked JSON index. |
| Replacement               | Pages write before the index publishes. A failed export leaves the prior index usable; old page directories are retained until removed manually.            |
| Memory                    | JSON pages stream through a buffered writer; decoding/analysis/aggregates still live in memory. Limits count records, not bytes.                            |
| Small in-process exports  | `Report::to_json()` stays a complete document with one shared event table; the CLI uses paged export instead.                                               |

| Export check (2026-09-13) | Result                                                                                                         |
| ------------------------- | -------------------------------------------------------------------------------------------------------------- |
| Synthetic 30 minutes      | 18,000 events, 1,800 inputs, 1,800 deliberately overlapping findings — a duplication stress case, not typical. |
| Output                    | 2.87MB total (725KB HTML, 4.4KB index). Unpaged repeated evidence estimated at 2.78GB.                         |
| Process                   | ~88ms report construction/export, 14.5MB peak RSS (debug build) — excludes decoding/detectors.                 |
| TV capture                | All 42 events, including seven React commits, recovered from the event pages.                                  |
| Reproduce                 | `cargo run -p session-telemetry-report --example report_size -- /tmp/rnst-report-size`                         |

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
