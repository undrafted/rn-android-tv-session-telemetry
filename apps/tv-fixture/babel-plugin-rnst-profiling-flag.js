// Replaces `__RN_SESSION_TELEMETRY_ENABLED__` and `__RN_SESSION_TELEMETRY_BENCHMARK__` with
// boolean literals at bundle time, based on the RNST_PROFILING/RNST_BENCHMARK env vars set
// before invoking Metro (RNST_PROFILING by the profiling Gradle build type; RNST_BENCHMARK only
// for a deliberate one-off overhead-benchmark run — see apps/tv-fixture's "benchmark" npm
// script). This lets Metro's minifier dead-code-eliminate the gated branches entirely in
// ordinary bundles, so production builds compile out the integration (and every bundle compiles
// out the benchmark branch) rather than just hiding either behind a runtime `if`.
module.exports = function rnstProfilingFlagPlugin({ types: t }) {
  // Quoted string keys, deliberately - a bare identifier key here (e.g.
  // `__RN_SESSION_TELEMETRY_ENABLED__: ...`) parses as an `Identifier` node, which is exactly
  // what this file's own visitor matches. Since this same plugin also transforms its own source
  // whenever something requires it through Jest's babel-jest transform (unlike the real
  // Metro/Gradle path, where plugin files load via plain Node `require` and never get
  // re-transformed), a bare key here would make the plugin try to replace its own object key
  // with a boolean literal and crash. A quoted key parses as `StringLiteral` instead, so it's
  // never a match — this isn't just a style choice.
  const flags = {
    '__RN_SESSION_TELEMETRY_ENABLED__': process.env.RNST_PROFILING === '1',
    '__RN_SESSION_TELEMETRY_BENCHMARK__': process.env.RNST_BENCHMARK === '1',
  };

  return {
    name: 'rnst-profiling-flag',
    visitor: {
      Identifier(path) {
        const name = path.node.name;
        if (
          Object.prototype.hasOwnProperty.call(flags, name) &&
          !path.scope.hasBinding(name)
        ) {
          path.replaceWith(t.booleanLiteral(flags[name]));
        }
      },
    },
  };
};
