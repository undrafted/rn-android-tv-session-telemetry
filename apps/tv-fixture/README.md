# TV Fixture

The demonstration Android TV app used to build and verify RN Session Telemetry against a real
device — a focusable card that records remote input, focus, and interaction-marker events
through the library.

## Development

```sh
npm start        # Metro dev server
npm run android  # build + install the profiling variant on a connected device/emulator
```

## Overhead benchmark

`npm run benchmark` (`RNST_BENCHMARK=1`) runs `SessionTelemetry.mark()` 5,000 times through the
disabled no-op path, then 5,000 more through the real active path (buffer, native bridge, JNI,
Rust chunk writer) — see `benchmark.ts`. The result is logged via `console.log` (read off `adb
logcat`) rather than written to a file: this is a one-off measurement, not part of a recorded
session, and kept as a separate launch mode specifically so it can never pollute one.

Measured 4 times on an Android TV emulator (arm64, API 36), profiling build:

| Run | Disabled avg/call | Active avg/call |
| --- | --- | --- |
| 1 | 0.000118 ms | 0.02658 ms |
| 2 | 0.000115 ms | 0.03470 ms |
| 3 | 0.000092 ms | 0.02817 ms |
| 4 | 0.000086 ms | 0.03009 ms |

The disabled-path number backs up the library's typed no-op claim: a non-profiling build pays
effectively nothing to have the library linked in.

Caveats, stated plainly: a tight loop, not spread-out real interactions, so real per-event cost
in normal use may differ (JIT warm-up, GC pauses); one emulator, not a real TV or a device
fleet; `mark()` specifically — other event types carry different JSON payload sizes and will
cost somewhat differently.
