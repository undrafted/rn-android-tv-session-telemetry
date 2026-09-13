// Ordinary application work: the blocked variant monopolizes JS, while the control waits
// asynchronously for the same duration. Neither variant records telemetry directly.
export function runStallScenario(blocking: boolean): Promise<void> {
  return new Promise(resolve => {
    setTimeout(() => {
      if (blocking) {
        const end = performance.now() + 250;
        while (performance.now() < end) {
          // Deliberately keep the event loop occupied for the fixture's blocked case.
        }
        resolve();
      } else {
        setTimeout(resolve, 250);
      }
    }, 20);
  });
}
