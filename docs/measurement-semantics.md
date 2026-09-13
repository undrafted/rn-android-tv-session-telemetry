### Measurement semantics

| Topic | Detail |
| --- | --- |
| Clocks | Monotonic-source only (`performance.now()`); wall-clock never drives duration math |
| Units | Every `durationMs`/`timestamp` field is milliseconds; `sequence` is a per-session monotonic counter, not a wall-clock-derived id |
| Overhead benchmark | See [apps/tv-fixture/README.md](../apps/tv-fixture/README.md#overhead-benchmark) |
| Event loss | `Chunk.manifest.lossCount` — always `0`, not wired up yet |
| Clock uncertainty | `ClockMap.uncertainty_ms` — largest residual across fitted clock-sync samples, a bound on how far a mapped bookmark timestamp could be off; computed but not yet surfaced in the CLI or HTML report |
| Storage budget | `SessionWriterModule.DEFAULT_BUDGET_BYTES` — 200 MB, unvalidated placeholder |

Detector thresholds (`session-telemetry-analysis::detector`) — all placeholders pending real
device measurements, not values anyone has validated against actual TV hardware yet:

| Detector | Trigger | Severity |
| --- | --- | --- |
| `high-latency-focus-change` | input→focus latency | Warning ≥100ms, Critical ≥300ms |
| `repeated-redux-dispatch` | same action type ≥2× in one interaction | Warning |
| `js-stall-during-interaction` | any captured stall | Warning, Critical ≥150ms |
| `repeated-network-request` | ≥2 equivalent requests in one interaction (streams ≥2000ms excluded) | Warning |
| `react-commit-overlapping-delayed-frame` | commit span overlaps a delayed frame's span | Warning |
| `network-completion-followed-by-commit` | a commit is the very next event after a network response | Warning |
| `excessive-commits-during-rapid-focus-movement` | ≥3 commits across a rapid-focus burst (gaps ≤150ms) | Warning |
