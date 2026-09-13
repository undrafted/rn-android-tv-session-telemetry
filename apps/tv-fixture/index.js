/**
 * @format
 */

import { AppRegistry } from 'react-native';
import { SessionTelemetry, startFrameTimingMonitor } from '@rn-session-telemetry/react-native';
import { runOverheadBenchmark } from './benchmark';
import App from './App';
import { name as appName } from './app.json';

// react-native-tvos's dev-mode AppContainer references a bare `window` global (for the React
// DevTools hook) that Hermes doesn't polyfill on this RN version - without this, the app
// crashes before rendering anything at all in a dev/debug build.
if (typeof global.window === 'undefined') {
  global.window = global;
}

if (__RN_SESSION_TELEMETRY_ENABLED__) {
  if (__RN_SESSION_TELEMETRY_BENCHMARK__) {
    // A dedicated one-off run (RNST_BENCHMARK=1, see the "benchmark" npm script), not something
    // that happens on an ordinary profiling launch - runOverheadBenchmark() drives its own
    // install()/stop() cycle internally, which would otherwise collide with (and pollute) the
    // real session this branch's `else` starts. Logged, not written to a file: this is a
    // one-off measurement read off logcat, not part of the recorded session's own data.
    const result = runOverheadBenchmark();
    // eslint-disable-next-line no-console
    console.log('RNST_OVERHEAD_BENCHMARK', JSON.stringify(result));
  } else {
    // This branch only runs in a real profiling build (RNST_PROFILING=1, see
    // babel-plugin-rnst-profiling-flag.js), so 'profiling' here is accurate, not guessed.
    SessionTelemetry.install({ buildType: 'profiling' });
    startFrameTimingMonitor();
  }
}

AppRegistry.registerComponent(appName, () => App);
