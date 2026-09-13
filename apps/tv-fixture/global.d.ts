// Injected by babel-plugin-rnst-profiling-flag.js at bundle time; see babel.config.js.
declare const __RN_SESSION_TELEMETRY_ENABLED__: boolean;
declare const __RN_SESSION_TELEMETRY_BENCHMARK__: boolean;

// React Native polyfills a monotonic `performance.now()` (Hermes/JSC), but its type
// definitions don't declare the global — only the piece benchmark.ts actually relies on. Same
// declaration as packages/react-native/src/global.d.ts, duplicated rather than shared because
// this is a separate TypeScript project with its own `include`.
declare const performance: {
  now(): number;
};
