// Two equivalent requests expose accidental duplicate loading; the second card makes one.
// These are ordinary application requests. Telemetry is enabled only once, in index.js.
// Run the local server and `adb reverse tcp:8787 tcp:8787` before selecting a card.
export async function loadNetworkScenario(duplicate: boolean): Promise<string> {
  const load = async () => {
    const response = await fetch(
      'http://127.0.0.1:8787/items?token=fixture-secret',
      {
        headers: { Authorization: 'Bearer fixture-secret' },
      },
    );
    if (!response.ok) throw new Error(`HTTP ${response.status}`);
    return response.text();
  };
  const results = await Promise.all(duplicate ? [load(), load()] : [load()]);
  return `${results.length} request${results.length === 1 ? '' : 's'} complete`;
}
