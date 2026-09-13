/**
 * RN Session Telemetry — TV fixture
 *
 * Two side-by-side focusable cards used to validate that a remote-control press, the resulting
 * focus transition, and its visible-update confirmation all show up as events — a DPAD_RIGHT/
 * DPAD_LEFT press has somewhere real to move focus to, unlike a single-card screen where no
 * focus change could ever occur. Later grows into a deliberately inefficient demonstration app.
 *
 * @format
 */

import { useState } from 'react';
import { StyleSheet, Text, View } from 'react-native';
import { FocusableView, SessionTelemetry } from '@rn-session-telemetry/react-native';

function Card({ id, label, hasTVPreferredFocus }: { id: string; label: string; hasTVPreferredFocus?: boolean }) {
  const [focused, setFocused] = useState(false);

  return (
    <FocusableView
      id={id}
      hasTVPreferredFocus={hasTVPreferredFocus}
      onFocus={() => setFocused(true)}
      onBlur={() => setFocused(false)}
      onPress={() => SessionTelemetry.mark(`demo:${id}-select`)}
      style={[styles.card, focused && styles.cardFocused]}
    >
      <Text style={styles.label}>{label}</Text>
    </FocusableView>
  );
}

function App() {
  return (
    <View style={styles.container}>
      <Card id="card-1" label="Press select" hasTVPreferredFocus />
      <Card id="card-2" label="Or move here" />
    </View>
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
});

export default App;
