# TV Fixture

The demonstration Android TV app used to build and verify RN Session Telemetry against a real
device — a focusable card that records remote input, focus, and interaction-marker events
through the library.

## Setup

```sh
npm install                                  # from the repo root — npm workspaces
rustup target add aarch64-linux-android      # session-telemetry-android's compile target
cargo install cargo-ndk                      # cross-compiles it into this app's jniLibs
```

Also needs an Android SDK + NDK (`ANDROID_NDK_HOME` or the SDK's bundled `ndk/`) and a connected
device or running emulator — the Gradle build shells out to `cargo ndk` on every build
(`packages/react-native/android/build.gradle`'s `cargoNdkBuildSessionWriter` task).

## Development

```sh
npm start        # Metro dev server
npm run android  # build + install the profiling variant on a connected device/emulator
```

## Overhead benchmark

Start the local endpoint from the repository root, then expose it to the device:

```sh
node apps/tv-fixture/scripts/network-server.cjs
adb reverse tcp:8787 tcp:8787
```

In another terminal, run `npm run benchmark --workspace=tv-fixture`. This builds the
profiling variant with `RNST_PROFILING=1 RNST_BENCHMARK=1`. When using Gradle directly,
pass both variables and `--no-daemon` so Metro receives the environment. The dedicated
benchmark screen replaces the ordinary fixture for this build.

Read results with `adb logcat -d -s ReactNativeJS`. Each `RNST_BENCHMARK_CASE` line contains
one case's JSON; `RNST_BENCHMARK_CONTEXT` records settings, device, and limits.
`RNST_OVERHEAD_BENCHMARK_COMPLETE` confirms success. An error produces
`RNST_OVERHEAD_BENCHMARK_ERROR` instead. The network endpoint must be available, and
active network/render cases must record exactly the expected number of events.

| Case                                                              | Measurement                                                                                                                                                                                                                                            |
| ----------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `mark`, `redux-dispatch`, `redux-selector`                        | JS time per call with recording stopped and active; Redux wrappers remain installed in both phases.                                                                                                                                                    |
| `react-callback`, `stall-event`                                   | JS time to submit synthetic events through the real recording API.                                                                                                                                                                                     |
| `network-fetch`                                                   | Sequential local requests through RN fetch, including response consumption and transport time.                                                                                                                                                         |
| `idle-timer`                                                      | Lateness beyond a requested 500ms wait, with the automatic stall sampler stopped and active. This is not CPU usage.                                                                                                                                    |
| `react-render`                                                    | Updates of 40 text nodes through the layout effect; unwrapped/stopped versus wrapped/recording. Both use the profiling renderer.                                                                                                                       |
| `resource-sampling-window-500ms`, `resource-sampling-window-16ms` | Lateness beyond a requested 2000ms wait, with a resource-sampling window closed versus open at the given interval, session already installed in both phases. This measures scheduling contention from the native sampler thread, not its own CPU time. |

Defaults: five rounds, 500 calls, ten requests, and twenty render updates per phase.
Each phase warms up and yields before measurement. Phase order alternates across rounds.
Results retain raw samples, event counts, phase medians, and the median paired delta.
Negative deltas remain visible because scheduling and transport noise can exceed the cost
being measured. Compare repeated device runs; emulator numbers are not a TV performance budget.

Install/stop and warm-up are outside the timed interval. These runs measure JS elapsed
time, not completion of queued native writes. They do not measure production-versus-profiling
renderer overhead, CPU, memory, or native focus/frame listener overhead. Each active phase
creates a native session containing benchmark events; use a test device. Rebuild without
`RNST_BENCHMARK` to restore the ordinary fixture.

### Benchmark results — 2026-09-13

Android TV emulator (`sdk_google_atv64_arm64`, arm64, API 36), profiling build.
Five alternating active/stopped rounds with warm-up; 500 calls, ten requests,
twenty render updates, or a 500ms idle wait per phase, depending on the case
(the two resource-sampling cases below use a 2000ms wait instead). This run
includes the two resource-sampling cases added alongside the resource-sampling
window feature and supersedes the earlier same-day figures for the other eight cases.

| Case                             | Unit                                    | Stopped median | Active median | Median paired delta |
| -------------------------------- | --------------------------------------- | -------------: | ------------: | ------------------: |
| `mark`                           | ms/call                                 |       0.000258 |      0.039143 |            0.039028 |
| `redux-dispatch`                 | ms/call                                 |       0.001840 |      0.040095 |            0.038484 |
| `redux-selector`                 | ms/call                                 |       0.001547 |      0.031252 |            0.029462 |
| `stall-event`                    | ms/call                                 |       0.000271 |      0.043068 |            0.042703 |
| `react-callback`                 | ms/call                                 |       0.000877 |      0.032559 |            0.029376 |
| `network-fetch`                  | ms/request including transport          |      19.996767 |     17.926096 |           -2.035342 |
| `idle-timer`                     | ms beyond requested wait                |      17.012667 |     15.924917 |           -1.389209 |
| `react-render`                   | ms/update through layout effect         |       1.205442 |      1.285535 |            0.138219 |
| `resource-sampling-window-500ms` | ms timer lateness beyond requested wait |      15.742251 |     19.205584 |            3.616875 |
| `resource-sampling-window-16ms`  | ms timer lateness beyond requested wait |      16.186501 |     18.927292 |            2.393291 |

The paired delta is the median of each round's active-minus-stopped difference,
not the difference between the two phase medians. Negative deltas in this run
do not establish a speedup. Network transport and scheduling noise are included;
idle lateness is not sampler CPU cost. These are one emulator run's measurements,
not production overhead limits.

The two resource-sampling cases both show a small, real positive delta — opening
a window costs a few milliseconds of extra JS-thread scheduling lateness across a
2-second window, at both the default 500ms interval (4 samples taken) and a
stress-test 16ms interval (~110 samples taken, near the 16ms clamp floor). The
16ms case's delta is not clearly larger than the 500ms case's despite taking
roughly 27x more samples — plausibly because this technique's own noise floor
(scheduling/GC jitter already visible in every other case's negative deltas)
dominates at this scale, not because sampling more often is free. This measures
scheduling contention only, not the native sampler thread's own CPU time; a
device without free cores to spare could show more contention than this run does.

Raw samples from this run are stored locally in
`pulled-sessions/overhead-benchmark/results.json` (not tracked in git).
