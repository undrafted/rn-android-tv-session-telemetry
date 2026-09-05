// React Native guarantees a monotonic `performance.now()` polyfill (backed by Hermes/JSC).
// Session events must use it, never `Date.now()` — see plan.md section 8.5: wall-clock time
// is metadata only and must never be used for duration calculations.
export function monotonicNowMs(): number {
  return performance.now();
}

export function createSequenceCounter(): () => number {
  let next = 0;
  return () => next++;
}
