// React Native polyfills a monotonic `performance.now()` (Hermes/JSC), but its type
// definitions don't declare the global — only the pieces we actually rely on.
declare const performance: {
  now(): number;
};

// RN/Hermes polyfills TextEncoder for the fetch/network instrumentation path; again only the
// piece actually used.
declare class TextEncoder {
  encode(input: string): Uint8Array;
}
