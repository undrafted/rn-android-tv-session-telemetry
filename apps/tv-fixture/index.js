/**
 * @format
 */

import { AppRegistry } from 'react-native';
import { SessionTelemetry } from '@rn-session-telemetry/react-native';
import App from './App';
import { name as appName } from './app.json';

if (__RN_SESSION_TELEMETRY_ENABLED__) {
  SessionTelemetry.install();
}

AppRegistry.registerComponent(appName, () => App);
