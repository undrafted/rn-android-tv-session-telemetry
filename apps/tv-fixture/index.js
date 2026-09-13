/**
 * @format
 */

import { AppRegistry } from 'react-native';
import { SessionTelemetry } from '@rn-session-telemetry/react-native';
import App from './App';
import { name as appName } from './app.json';

// react-native-tvos's dev-mode AppContainer references a bare `window` global (for the React
// DevTools hook) that Hermes doesn't polyfill on this RN version - without this, the app
// crashes before rendering anything at all in a dev/debug build.
if (typeof global.window === 'undefined') {
  global.window = global;
}

if (__RN_SESSION_TELEMETRY_ENABLED__) {
  SessionTelemetry.install();
}

AppRegistry.registerComponent(appName, () => App);
