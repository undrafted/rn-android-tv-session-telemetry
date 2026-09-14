/** @format */
const path = require('node:path');
const {
  withReactProfiling,
} = require('@rn-android-tv-session-telemetry/react-native/metro.cjs');
const root = path.dirname(require.resolve('react-native/package.json'));
const prod = path.join(
  root,
  'Libraries/Renderer/implementations/ReactFabric-prod.js',
);

it('does not alter disabled builds or require a profiling runtime for them', () => {
  const config = { resolver: { resolveRequest: jest.fn() } };
  expect(withReactProfiling(config)).toBe(config);
});
it('preserves existing resolution and replaces only the pinned production Fabric renderer', () => {
  const result = { type: 'sourceFile', filePath: prod };
  const previous = jest.fn(() => result);
  const config = withReactProfiling(
    { resolver: { resolveRequest: previous } },
    { enabled: true, reactNativePath: root },
  );
  const context = {};
  expect(
    config.resolver.resolveRequest(
      context,
      '../implementations/ReactFabric-prod',
      'android',
    ).filePath,
  ).toBe(prod.replace('-prod.js', '-profiling.js'));
  expect(previous).toHaveBeenCalledWith(
    context,
    '../implementations/ReactFabric-prod',
    'android',
  );
  result.filePath = '/other/package/ReactFabric-prod.js';
  expect(config.resolver.resolveRequest(context, 'other', 'android')).toBe(
    result,
  );
});
it('rejects unsupported explicitly requested profiling runtimes', () => {
  expect(() => withReactProfiling({}, { enabled: true })).toThrow(
    'explicit reactNativePath',
  );
  expect(() =>
    withReactProfiling(
      {},
      {
        enabled: true,
        reactNativePath: path.dirname(require.resolve('react/package.json')),
      },
    ),
  ).toThrow('react-native-tvos 0.87.1-0 only');
});
