/**
 * RN Session Telemetry — TV fixture
 *
 * Two side-by-side focusable cards used to validate that a remote-control press, the resulting
 * focus transition, and its visible-update confirmation all show up as events — a DPAD_RIGHT/
 * DPAD_LEFT press has somewhere real to move focus to, unlike a single-card screen where no
 * focus change could ever occur. Focus/visible-update capture is zero-footprint: a plain
 * `Pressable` with `nativeID` is all that's needed (see index.js's startGlobalFocusMonitor()) —
 * no telemetry-specific wrapper component or onFocus/onBlur handler required. Also wired to a
 * minimal Redux store (store.ts) demonstrating selector instrumentation. Later grows into a
 * deliberately inefficient demonstration app. A second row compares synchronous JS blocking
 * with an asynchronous wait of the same duration, without telemetry calls in either action.
 *
 * @format
 */

import { useState } from 'react';
import { loadNetworkScenario } from './networkScenario';
import { runStallScenario } from './stallScenario';
import { runResourceHeavyScenario } from './resourceSamplingScenario';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { SessionTelemetry } from '@rn-session-telemetry/react-native';
import {
  selectItemCount,
  selectVisibleItemIds,
  store,
  useAppSelector,
} from './store';

function Card({
  id,
  label,
  hasTVPreferredFocus,
}: {
  id: string;
  label: string;
  hasTVPreferredFocus?: boolean;
}) {
  // Local visual state only — unrelated to telemetry, which the native global focus listener
  // now handles entirely on its own via this Pressable's nativeID.
  const [focused, setFocused] = useState(false);
  const [networkStatus, setNetworkStatus] = useState('Select to load');
  // Subscribes to the store via the *stable* selector only — useSyncExternalStore requires its
  // snapshot to be reference-stable when nothing relevant changed, which selectVisibleItemIds
  // deliberately isn't (that's the bug this fixture demonstrates). Calling the unstable one
  // directly, as a plain derived value during render, is exactly how this misuse actually shows
  // up in real apps: called directly instead of through a properly memoized selector hook.
  const itemCount = useAppSelector(selectItemCount);
  const visibleItemIds = selectVisibleItemIds(store.getState());

  return (
    <Pressable
      nativeID={id}
      hasTVPreferredFocus={hasTVPreferredFocus}
      onFocus={() => {
        setFocused(true);
        store.dispatch({ type: 'catalog/cardFocused', targetId: id });
      }}
      onBlur={() => setFocused(false)}
      onPress={() => {
        SessionTelemetry.mark(`demo:${id}-select`);
        setNetworkStatus('Loading…');
        loadNetworkScenario(id === 'card-1').then(setNetworkStatus, () =>
          setNetworkStatus('Start the local network server'),
        );
      }}
      style={[styles.card, focused && styles.cardFocused]}
    >
      <Text style={styles.label}>{label}</Text>
      <Text style={styles.detail}>{networkStatus}</Text>
      <Text style={styles.detail}>
        {itemCount} items · {visibleItemIds.length} visible
      </Text>
    </Pressable>
  );
}

function StallCard({ blocking }: { blocking: boolean }) {
  const [focused, setFocused] = useState(false);
  const [status, setStatus] = useState('Select to run');
  return (
    <Pressable
      nativeID={blocking ? 'stall-blocked' : 'stall-yielding'}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      onPress={() => {
        setStatus('Running…');
        runStallScenario(blocking).then(() => setStatus('Complete'));
      }}
      style={[styles.card, focused && styles.cardFocused]}
    >
      <Text style={styles.label}>
        {blocking ? 'Block JS · 250ms' : 'Yield JS · 250ms'}
      </Text>
      <Text style={styles.detail}>{status}</Text>
    </Pressable>
  );
}

function CommitCard({ slow }: { slow: boolean }) {
  const [count, setCount] = useState(0);
  const [focused, setFocused] = useState(false);
  // Deliberately expensive render work, compared with an ordinary state update beside it.
  // The root profiler measures this automatically; the component has no telemetry calls.
  if (slow && count > 0) {
    const end = performance.now() + 40;
    while (performance.now() < end) {
      // Keep this render busy for the pathological fixture case.
    }
  }
  return (
    <Pressable
      nativeID={slow ? 'commit-slow' : 'commit-fast'}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      onPress={() => setCount(value => value + 1)}
      style={[styles.card, focused && styles.cardFocused]}
    >
      <Text style={styles.label}>
        {slow ? 'Slow render · 40ms' : 'Fast render'}
      </Text>
      <Text style={styles.detail}>Updates: {count}</Text>
    </Pressable>
  );
}

function ResourceCard() {
  const [focused, setFocused] = useState(false);
  const [status, setStatus] = useState('Select to stream');
  return (
    <Pressable
      nativeID="resource-heavy"
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      onPress={() => {
        // Explicit, app-toggled sampling window - off by default, opened only around this
        // scenario, same posture as wiring in Redux middleware. intervalMs is short enough
        // that this ~1.5s scenario still yields several samples.
        SessionTelemetry.startResourceSampling({ intervalMs: 200 });
        setStatus('Streaming…');
        runResourceHeavyScenario().then(() => {
          SessionTelemetry.stopResourceSampling();
          setStatus('Complete');
        });
      }}
      style={[styles.card, focused && styles.cardFocused]}
    >
      <Text style={styles.label}>Stream/playback</Text>
      <Text style={styles.detail}>{status}</Text>
    </Pressable>
  );
}

function App() {
  return (
    <View style={styles.container}>
      <View style={styles.row}>
        <Card id="card-1" label="Load twice" hasTVPreferredFocus />
        <Card id="card-2" label="Load once" />
      </View>
      <View style={styles.row}>
        <StallCard blocking />
        <StallCard blocking={false} />
      </View>
      <View style={styles.row}>
        <CommitCard slow />
        <CommitCard slow={false} />
      </View>
      <View style={styles.row}>
        <ResourceCard />
      </View>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    flexDirection: 'column',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 24,
    backgroundColor: '#0b0b0f',
  },
  row: {
    flexDirection: 'row',
    gap: 24,
  },
  card: {
    width: 240,
    height: 135,
    borderRadius: 8,
    backgroundColor: '#1c1c24',
    alignItems: 'center',
    justifyContent: 'center',
  },
  cardFocused: {
    backgroundColor: '#3a63ff',
  },
  label: {
    color: '#ffffff',
    fontSize: 18,
  },
  detail: {
    color: '#9a9aa8',
    fontSize: 12,
    marginTop: 4,
  },
});

export default App;
