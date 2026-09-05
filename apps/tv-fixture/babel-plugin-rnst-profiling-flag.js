// Replaces every `__RN_SESSION_TELEMETRY_ENABLED__` identifier with a boolean literal at
// bundle time, based on the RNST_PROFILING env var the profiling Gradle build type is
// expected to set before invoking Metro. This lets Metro's minifier dead-code-eliminate the
// `if (__RN_SESSION_TELEMETRY_ENABLED__) { ... }` branch entirely in non-profiling bundles,
// satisfying the "production builds must compile out the integration" requirement in
// plan.md section 5, rather than just hiding it behind a runtime `if`.
module.exports = function rnstProfilingFlagPlugin({ types: t }) {
  const enabled = process.env.RNST_PROFILING === '1';

  return {
    name: 'rnst-profiling-flag',
    visitor: {
      Identifier(path) {
        if (
          path.node.name === '__RN_SESSION_TELEMETRY_ENABLED__' &&
          !path.scope.hasBinding('__RN_SESSION_TELEMETRY_ENABLED__')
        ) {
          path.replaceWith(t.booleanLiteral(enabled));
        }
      },
    },
  };
};
