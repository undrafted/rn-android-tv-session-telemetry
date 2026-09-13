### Measurement semantics

Clocks: monotonic-source only (`performance.now()`); wall-clock never drives duration math.

`SessionTelemetry.mark()` overhead — 5000 iterations × 4 runs, TV emulator (arm64, API 36),
profiling build:

| | Disabled | Active |
| --- | --- | --- |
| Avg/call | ~0.0001 ms | 0.026–0.035 ms |

_(`runOverheadBenchmark()`, run via `npm run benchmark` in apps/tv-fixture)_

`Chunk.manifest.lossCount`: always 0, not wired up yet.
`SessionWriterModule.DEFAULT_BUDGET_BYTES`: 200 MB, unvalidated placeholder.
