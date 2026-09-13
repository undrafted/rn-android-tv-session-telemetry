/**
 * @format
 */

const { transformSync } = require('@babel/core');
const plugin = require('./babel-plugin-rnst-profiling-flag');

const SOURCE = `
if (__RN_SESSION_TELEMETRY_ENABLED__) { install(); }
if (__RN_SESSION_TELEMETRY_BENCHMARK__) { benchmark(); }
`;

function setEnvVar(name, value) {
  if (value === undefined) {
    delete process.env[name];
  } else {
    process.env[name] = value;
  }
}

// The plugin reads process.env at construction time, so it must be re-required (a fresh module
// instance, not the cached one) after each env change this test exercises. Under Jest, that
// means `jest.resetModules()` — plain Node `require.cache` manipulation doesn't touch Jest's own
// per-test module registry, so a `delete require.cache[...]` here would silently keep returning
// the first-ever plugin instance regardless of env changes.
function transformWithEnv(profiling, benchmark) {
  const previousProfiling = process.env.RNST_PROFILING;
  const previousBenchmark = process.env.RNST_BENCHMARK;
  setEnvVar('RNST_PROFILING', profiling);
  setEnvVar('RNST_BENCHMARK', benchmark);
  try {
    jest.resetModules();
    const freshPlugin = require('./babel-plugin-rnst-profiling-flag');
    return transformSync(SOURCE, {
      plugins: [freshPlugin],
      configFile: false,
      babelrc: false,
    }).code;
  } finally {
    setEnvVar('RNST_PROFILING', previousProfiling);
    setEnvVar('RNST_BENCHMARK', previousBenchmark);
  }
}

test('both flags compile to false when neither env var is set', () => {
  const code = transformWithEnv(undefined, undefined);

  expect(code).toContain('if (false) {\n  install();\n}');
  expect(code).toContain('if (false) {\n  benchmark();\n}');
});

test('RNST_PROFILING=1 flips only the enabled flag', () => {
  const code = transformWithEnv('1', undefined);

  expect(code).toContain('if (true) {\n  install();\n}');
  expect(code).toContain('if (false) {\n  benchmark();\n}');
});

test('RNST_BENCHMARK=1 flips only the benchmark flag', () => {
  const code = transformWithEnv(undefined, '1');

  expect(code).toContain('if (false) {\n  install();\n}');
  expect(code).toContain('if (true) {\n  benchmark();\n}');
});

test('does not touch a same-named local variable', () => {
  const code = transformSync(
    'function f() { const __RN_SESSION_TELEMETRY_ENABLED__ = 1; return __RN_SESSION_TELEMETRY_ENABLED__; }',
    { plugins: [plugin], configFile: false, babelrc: false },
  ).code;

  expect(code).not.toContain('true');
  expect(code).not.toContain('false');
});
