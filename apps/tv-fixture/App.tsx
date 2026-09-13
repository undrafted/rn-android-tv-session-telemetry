/**
 * RN Session Telemetry — TV fixture
 *
 * Minimal focusable screen used to validate that a remote-control press and a focus
 * transition both show up as events. Later grows into a deliberately inefficient
 * demonstration app.
 *
 * @format
 */

import { useState } from 'react';
import { Pressable, StyleSheet, Text, View } from 'react-native';
import { SessionTelemetry } from '@rn-session-telemetry/react-native';

function App() {
  const [focused, setFocused] = useState(false);

  return (
    <View style={styles.container}>
      <Pressable
        hasTVPreferredFocus
        onFocus={() => {
          setFocused(true);
          SessionTelemetry.recordFocus('demo-card');
        }}
        onBlur={() => setFocused(false)}
        onPress={() => SessionTelemetry.mark('demo:card-select')}
        style={[styles.card, focused && styles.cardFocused]}
      >
        <Text style={styles.label}>Press select</Text>
      </Pressable>
    </View>
  );
}

const styles = StyleSheet.create({
  container: {
    flex: 1,
    alignItems: 'center',
    justifyContent: 'center',
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
