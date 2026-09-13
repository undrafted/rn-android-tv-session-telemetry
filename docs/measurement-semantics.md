### Measurement semantics

See [signal flow](signals.md) for the capture and analysis diagram.

| Topic               | Behavior                                                                                                                                                                                                                                                      |
| ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Clocks              | Durations use monotonic clocks: `performance.now()` in JS and Android's native clock for frame timing. `ClockSyncEvent.wallClockUnixMs` maps workstation bookmarks onto the session timeline.                                                                 |
| Units               | Timestamps and durations are milliseconds. Sequence numbers are counters.                                                                                                                                                                                     |
| Sequence numbers    | One counter per JS process. `install()` and `stop()` do not reset it; restarting the app does.                                                                                                                                                                |
| Session boundary    | App calls and CLI start/stop broadcasts can open or seal the native session. Starting a session seals any open session first.                                                                                                                                 |
| Startup ordering    | `startNativeSession()` runs before metadata and clock-sync events are emitted. The native writer ignores pushes while no session is open.                                                                                                                     |
| Overhead benchmark  | Compares average `mark()` cost with recording stopped and active. Results come from logcat, outside the recorded session. See the [benchmark instructions](../apps/tv-fixture/README.md#overhead-benchmark).                                                  |
| Event loss          | `Chunk.manifest.lossCount` is always `0` for now. Sequence-gap detection is not implemented, so this value does not establish that no events were lost.                                                                                                       |
| Clock mapping       | `ClockMap` fits offset and scale by least squares. One sample gives an offset with scale `1.0`. `uncertainty_ms` is the largest fitted residual, not a guaranteed error bound; it is `0.0` with one sample. Reports use `null` when no samples are available. |
| Storage budget      | Chunks rotate at 256 KB or 60 seconds. The session budget is 200 MB. Reaching it stops writes for that session and logs once; sealed chunks remain available. These defaults still need device validation.                                                    |
| Native registration | `RnSessionTelemetryPackage` registers all native modules through autolinking. Add new modules to its `createNativeModules()` list.                                                                                                                            |
| Format              | Recordings and reports are unversioned during development. Update the collector and CLI together; compatibility with older captures is not guaranteed.                                                                                                        |

### React timing

| Field                                             | Meaning                                                                                                                                      |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `actualDurationMs`                                | Render work for the committed subtree.                                                                                                       |
| `baseDurationMs`                                  | React's estimate of the subtree's render cost without memoization.                                                                           |
| `renderStartMs`                                   | When React started rendering the update.                                                                                                     |
| `commitTimeMs`                                    | React's commit timestamp.                                                                                                                    |
| `timestamp`                                       | When the telemetry callback observed the commit.                                                                                             |
| `reactCommitCapture: observed`                    | At least one commit was recorded. Capture may still be incomplete.                                                                           |
| `reactCommitCapture: unavailable-or-not-observed` | No commit was recorded. Possible causes include an unsupported renderer, an unwrapped root, inactive capture, or no commit during recording. |

Render-to-commit elapsed time can include pauses. It is separate from render work,
commit-phase duration, and frame presentation time. Frame-overlap analysis requires valid
renderer timestamps; missing or invalid intervals are excluded. Overlap alone does not
establish causation.

| Setup             | Behavior                                                                                                                                                                      |
| ----------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Root registration | `withTelemetryRoot(App, appName)` wraps the app in a library-owned `Profiler` in profiling builds.                                                                            |
| Renderer          | `withReactProfiling` selects the Fabric profiling renderer in Metro. The production renderer does not emit these callbacks.                                                   |
| Lifecycle         | `stop()` disables recording but keeps the root mounted to preserve state. Reinstall resumes recording through the same wrapper.                                               |
| Scope             | Tested with Android TV, Fabric/Hermes, RN TV 0.87.1-0, and React 19.2.3. Captures root commits, without component attribution or props/state values. Profiling adds overhead. |
| TV check          | Seven commits preserved `renderStartMs ≤ commitTimeMs ≤ timestamp` through storage and reports. The mount callback arrived about 21ms after React's commit timestamp.         |

### Signal capture

All signals below are exercised by the TV fixture.

| Signal             | Source and limits                                                                                                                                                                                                                                                                                                                                                                                                                                                       |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Remote input       | `TVEventHandler.addListener`, filtered by `isRemoteInputEventType`.                                                                                                                                                                                                                                                                                                                                                                                                     |
| Focus              | `GlobalFocusModule.kt` listens for app-wide focus changes and identifies views by their RN `nativeID`. No per-view telemetry handler is required.                                                                                                                                                                                                                                                                                                                       |
| Visible update     | The focus listener posts a callback to the focused view's UI-thread queue. This is an approximation of a visible update, not a frame-presentation timestamp.                                                                                                                                                                                                                                                                                                            |
| Interaction marker | Explicit `SessionTelemetry.mark()` calls.                                                                                                                                                                                                                                                                                                                                                                                                                               |
| Redux dispatch     | `createTelemetryMiddleware` from `packages/redux`.                                                                                                                                                                                                                                                                                                                                                                                                                      |
| Redux selector     | `telemetrySelector()` calls the selector once and returns its result. It compares arguments and results with the previous call using `Object.is`. Both change flags are `true` on the first call.                                                                                                                                                                                                                                                                       |
| Network request    | `install()` observes shared XHR traffic: RN fetch, direct XHR, and clients using the same XHR prototype. Independent/native transports are excluded. One terminal event records metadata; duration ends at XHR completion. Sizes are estimates or null. Credentials, fragments, and query parameters are removed except for allowlisted parameters; paths remain visible. See the [capture boundary](../diagrams/network.svg).                                          |
| JS stall           | `install()` starts one timer; reinstall replaces it and `stop()` clears it. The standalone helper also replaces the sampler. Defaults: 50ms interval and 50ms threshold. Events record overshoot strictly above the threshold. Intervals clamp to 16–2,147,483,647ms; negative thresholds become zero and non-finite options use defaults. Each tick sets a fresh deadline. No stacks are captured; GC, scheduling, and background suspension can also delay the timer. |
| React commit       | Root `Profiler` callback; see the timing fields above. The fixture includes slow and fast render actions.                                                                                                                                                                                                                                                                                                                                                               |
| Frame timing       | `FrameTimingModule.kt` reads `FrameMetrics.TOTAL_DURATION`. Only frames exceeding `thresholdMs` (32ms by default) are sent to JS.                                                                                                                                                                                                                                                                                                                                       |
| Clock sync         | First event after `install()`, then with the next pushed event once 30 seconds have elapsed since the previous sample.                                                                                                                                                                                                                                                                                                                                                  |
| Session metadata   | Emitted at `install()`. Device model and OS version come from RN `Platform`; app version and build type come from `InstallOptions`.                                                                                                                                                                                                                                                                                                                                     |

### Analysis

“Evidence only” means a signal can appear in a finding's event range without being used
by a detector.

| Signal             | Consumer                                                                                                                                                          |
| ------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Remote input       | Starts an `InteractionWindow`.                                                                                                                                    |
| Focus              | `high-latency-focus-change`                                                                                                                                       |
| Visible update     | `high-latency-visible-update`; ends the interaction window scan.                                                                                                  |
| Interaction marker | Evidence only                                                                                                                                                     |
| Redux dispatch     | `repeated-redux-dispatch`                                                                                                                                         |
| Redux selector     | `repeated-selector-recomputation`, `unstable-selector-reference`, and session-wide `Report.selectorStats`                                                         |
| Network request    | `repeated-network-request`, `network-completion-followed-by-commit`                                                                                               |
| JS stall           | `js-stall-during-interaction`                                                                                                                                     |
| React commit       | `react-commit-overlapping-delayed-frame`, `network-completion-followed-by-commit`, `excessive-commits-during-rapid-focus-movement`, `unstable-selector-reference` |
| Frame timing       | `react-commit-overlapping-delayed-frame`                                                                                                                          |
| Clock sync         | Bookmark mapping and `Report.clockUncertaintyMs`                                                                                                                  |
| Session metadata   | `Report.deviceMetadata`, using the first matching event                                                                                                           |

Thresholds in `session-telemetry-analysis::detector` are provisional and need calibration
on TV hardware.

| Detector                                        | Trigger                                                                                      | Severity                        |
| ----------------------------------------------- | -------------------------------------------------------------------------------------------- | ------------------------------- |
| `high-latency-focus-change`                     | Input-to-focus latency                                                                       | Warning ≥100ms; critical ≥300ms |
| `high-latency-visible-update`                   | Input-to-visible-update latency                                                              | Warning ≥100ms; critical ≥300ms |
| `repeated-redux-dispatch`                       | Same action type ≥2 times in one interaction                                                 | Warning                         |
| `js-stall-during-interaction`                   | Any captured stall                                                                           | Warning; critical ≥150ms        |
| `repeated-network-request`                      | ≥2 equivalent requests in one interaction; streams ≥2000ms excluded                          | Warning                         |
| `react-commit-overlapping-delayed-frame`        | Render-to-commit interval overlaps an estimated delayed-frame span                           | Warning                         |
| `network-completion-followed-by-commit`         | A commit is the next event after a network response                                          | Warning                         |
| `excessive-commits-during-rapid-focus-movement` | ≥3 commits across a focus burst with gaps ≤150ms                                             | Warning                         |
| `repeated-selector-recomputation`               | Same selector called ≥2 times with unchanged inputs in one interaction                       | Warning                         |
| `unstable-selector-reference`                   | New selector result reference with unchanged inputs, in a window containing ≥2 React commits | Warning                         |
