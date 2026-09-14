# TV Fixture

The demo Android TV app used to build and verify RN Android TV Session Telemetry against a real
device — a focusable card that records remote input, focus, and interaction-marker events
through the library.

## Setup

```sh
npm install                                  # from the repo root — npm workspaces
rustup target add aarch64-linux-android      # session-telemetry-android's compile target
cargo install cargo-ndk                      # cross-compiles it into this app's jniLibs
```

Also needs an Android SDK + NDK (`ANDROID_NDK_HOME` or the SDK's bundled `ndk/`) and a connected
device or emulator — the Gradle build shells out to `cargo ndk` on every build
(`packages/react-native/android/build.gradle`'s `cargoNdkBuildSessionWriter` task).

## Development

```sh
npm start        # Metro dev server
npm run android  # build + install the profiling variant on a connected device/emulator
```

## Overhead benchmark

Start the local endpoint, then expose it to the device:

```sh
node apps/tv-fixture/scripts/network-server.cjs
adb reverse tcp:8787 tcp:8787
```

Run `npm run benchmark --workspace=tv-fixture` (builds with `RNST_PROFILING=1
RNST_BENCHMARK=1`; pass both plus `--no-daemon` if using Gradle directly so Metro sees them).
The benchmark screen replaces the ordinary fixture for this build.

Read results with `adb logcat -d -s ReactNativeJS`: `RNST_BENCHMARK_CASE` lines hold each
case's JSON, `RNST_BENCHMARK_CONTEXT` holds settings/device/limits,
`RNST_OVERHEAD_BENCHMARK_COMPLETE` confirms success (`_ERROR` on failure). The network endpoint
must be running, and active network/render cases must record the expected event count.

| Case                                                              | Measurement                                                                                                                                    |
| ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `mark`, `redux-dispatch`, `redux-selector`                        | JS time per call, recording stopped vs. active; Redux wrappers stay installed both phases.                                                     |
| `react-callback`, `stall-event`                                   | JS time to submit synthetic events through the real recording API.                                                                             |
| `network-fetch`                                                   | Sequential local requests through RN fetch, including response consumption and transport.                                                      |
| `idle-timer`                                                      | Lateness beyond a requested 500ms wait, stall sampler stopped vs. active. Not CPU usage.                                                       |
| `react-render`                                                    | 40 text-node updates through the layout effect, unwrapped/stopped vs. wrapped/recording. Both use the profiling renderer.                      |
| `resource-sampling-window-500ms`, `resource-sampling-window-16ms` | Lateness beyond a requested 2000ms wait, resource-sampling window closed vs. open at the given interval — measures scheduling contention only. |

Defaults: five rounds, 500 calls, ten requests, twenty renders per phase; each phase warms up
and yields before measurement, phase order alternating across rounds. Results keep raw samples,
event counts, phase medians, and the median paired delta. Negative deltas stay visible —
scheduling/transport noise can exceed the cost being measured; compare repeated runs, and treat
emulator numbers as illustrative, not a TV performance budget.

Install/stop and warm-up sit outside the timed interval. These measure JS elapsed time, not
completion of queued native writes, production-vs-profiling renderer overhead, or native
focus/frame listener overhead. Each active phase creates a native session with benchmark
events — use a test device. Rebuild without `RNST_BENCHMARK` for the ordinary fixture.

### Benchmark results — 2026-09-13

Android TV emulator (`sdk_google_atv64_arm64`, arm64, API 36), profiling build. Five alternating
active/stopped rounds with warm-up; 500 calls / ten requests / twenty renders / a 500ms idle
wait per phase (2000ms for the two resource-sampling cases, added alongside that feature).

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

Paired delta is the median of each round's active-minus-stopped difference, not the gap between
phase medians. Negative deltas here don't establish a speedup — network/scheduling noise is
included, and idle lateness isn't sampler CPU cost. One emulator run, not a production limit.

Both resource-sampling cases show a small, real positive delta from opening a window: +3.6ms at
the 500ms default interval (4 samples/2s) and +2.4ms at a 16ms stress interval (~110
samples/2s). The 16ms case isn't clearly worse despite ~27x more samples — likely this
technique's own scheduling/GC noise floor dominates at this scale, not that denser sampling is
free. This measures scheduling contention only, not the sampler thread's own CPU time; a device
with fewer free cores could show more.

Raw samples: `pulled-sessions/overhead-benchmark/results.json` (not tracked in git).
