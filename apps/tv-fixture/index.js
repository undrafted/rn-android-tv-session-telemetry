/**
 * @format
 */

import { AppRegistry } from 'react-native';
import {
  SessionTelemetry,
  startFrameTimingMonitor,
  startGlobalFocusMonitor,
  withTelemetryRoot,
} from '@rn-android-tv-session-telemetry/react-native';
import BenchmarkApp from './BenchmarkApp';
import App from './App';
import { name as appName } from './app.json';

// react-native-tvos's dev-mode AppContainer references a bare `window` global (for the React
// DevTools hook) that Hermes doesn't polyfill on this RN version - without this, the app
// crashes before rendering anything at all in a dev/debug build.
if (typeof global.window === 'undefined') {
  global.window = global;
}

if (__RN_SESSION_TELEMETRY_ENABLED__) {
  if (!__RN_SESSION_TELEMETRY_BENCHMARK__) {
    // This branch only runs in a real profiling build (RNST_PROFILING=1, see
    // babel-plugin-rnst-profiling-flag.js), so 'profiling' here is accurate, not guessed.
    SessionTelemetry.install({ buildType: 'profiling' });
    startFrameTimingMonitor();
    startGlobalFocusMonitor();
  }
}

const Root = __RN_SESSION_TELEMETRY_BENCHMARK__
  ? BenchmarkApp
  : __RN_SESSION_TELEMETRY_ENABLED__
    ? withTelemetryRoot(App, appName)
    : App;
AppRegistry.registerComponent(appName, () => Root);
