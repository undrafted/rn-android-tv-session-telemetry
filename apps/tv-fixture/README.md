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

| Case                                       | Measurement                                                                                                                      |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------- |
| `mark`, `redux-dispatch`, `redux-selector` | JS time per call with recording stopped and active; Redux wrappers remain installed in both phases.                              |
| `react-callback`, `stall-event`            | JS time to submit synthetic events through the real recording API.                                                               |
| `network-fetch`                            | Sequential local requests through RN fetch, including response consumption and transport time.                                   |
| `idle-timer`                               | Lateness beyond a requested 500ms wait, with the automatic stall sampler stopped and active. This is not CPU usage.              |
| `react-render`                             | Updates of 40 text nodes through the layout effect; unwrapped/stopped versus wrapped/recording. Both use the profiling renderer. |

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
twenty render updates, or a 500ms idle wait per phase, depending on the case.

| Case             | Unit                            | Stopped median | Active median | Median paired delta |
| ---------------- | ------------------------------- | -------------: | ------------: | ------------------: |
| `mark`           | ms/call                         |       0.000250 |      0.027190 |            0.026767 |
| `redux-dispatch` | ms/call                         |       0.001423 |      0.036928 |            0.035219 |
| `redux-selector` | ms/call                         |       0.001410 |      0.028330 |            0.024999 |
| `stall-event`    | ms/call                         |       0.000317 |      0.037549 |            0.037233 |
| `react-callback` | ms/call                         |       0.000883 |      0.028118 |            0.027323 |
| `network-fetch`  | ms/request including transport  |      16.860683 |     16.769854 |           -0.285321 |
| `idle-timer`     | ms beyond requested wait        |      16.318834 |     15.191875 |           -0.434708 |
| `react-render`   | ms/update through layout effect |       1.145502 |      1.181087 |           -0.007975 |

The paired delta is the median of each round's active-minus-stopped difference,
not the difference between the two phase medians. Negative deltas in this run
do not establish a speedup. Network transport and scheduling noise are included;
idle lateness is not sampler CPU cost. These are one emulator run's measurements,
not production overhead limits.

Raw samples from this run are stored locally in
`pulled-sessions/overhead-benchmark/results.json` (not tracked in git).
