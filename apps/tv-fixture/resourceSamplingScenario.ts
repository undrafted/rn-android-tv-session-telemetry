// Simulates the kind of periodic decode-like CPU work a real stream/playback screen would do -
// ordinary application work, not telemetry itself. Chunked into several bursts separated by
// short yields rather than one long synchronous block, so this stays a representative workload
// (and a responsive app) instead of freezing the JS thread outright for two seconds straight.
export function runResourceHeavyScenario(): Promise<void> {
  return new Promise(resolve => {
    let ticks = 0;
    const totalTicks = 6;
    function tick() {
      const end = performance.now() + 200;
      while (performance.now() < end) {
        // Deliberately CPU-heavy busy work for this fixture's resource-sampling demo.
      }
      ticks += 1;
      if (ticks < totalTicks) {
        setTimeout(tick, 50);
      } else {
        resolve();
      }
    }
    tick();
  });
}
