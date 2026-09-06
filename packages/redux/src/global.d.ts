// React Native polyfills a monotonic `performance.now()` (Hermes/JSC), but its type
// definitions don't declare the global — only the piece we actually rely on.
declare const performance: {
  now(): number;
};
