/**
 * RN Session Telemetry — TV fixture
 *
 * Two side-by-side focusable cards used to validate that a remote-control press, the resulting
 * focus transition, and its visible-update confirmation all show up as events — a DPAD_RIGHT/
 * DPAD_LEFT press has somewhere real to move focus to, unlike a single-card screen where no
 * focus change could ever occur. Focus/visible-update capture is zero-footprint: a plain
 * `Pressable` with `nativeID` is all that's needed (see index.js's startGlobalFocusMonitor()) —
 * no telemetry-specific wrapper component or onFocus/onBlur handler required. Also wired to a
 * minimal Redux store (store.ts) demonstrating M7's selector instrumentation. Later grows into a
 * deliberately inefficient demonstration app.
 *
 * @format
 */

import { Profiler, useState } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import {
  SessionTelemetry,
  onProfilerRender,
} from '@rn-session-telemetry/react-native';
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
      onPress={() => SessionTelemetry.mark(`demo:${id}-select`)}
      style={[styles.card, focused && styles.cardFocused]}
    >
      <Text style={styles.label}>{label}</Text>
      <Text style={styles.detail}>
        {itemCount} items · {visibleItemIds.length} visible
      </Text>
    </Pressable>
  );
}

function App() {
  return (
    <Profiler id="App" onRender={onProfilerRender}>
      <View style={styles.container}>
        <Card id="card-1" label="Press select" hasTVPreferredFocus />
        <Card id="card-2" label="Or move here" />
      </View>
    </Profiler>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    flexDirection: 'row',
    alignItems: 'center',
    justifyContent: 'center',
    gap: 24,
    backgroundColor: '#0b0b0f',
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
