/* global require, module */
/* eslint-disable @typescript-eslint/no-require-imports -- Metro configuration is loaded as CommonJS. */
const path = require('node:path');
const fs = require('node:fs');

// The renderer filename is version-specific. Fail an explicitly requested profiling build
// if the pinned runtime changes, rather than silently shipping a renderer with no callbacks.
function withReactProfiling(config, { enabled = false, reactNativePath } = {}) {
  if (!enabled) return config;
  if (!reactNativePath) throw new Error('React profiling requires an explicit reactNativePath');
  const root = fs.realpathSync(reactNativePath);
  const metadata = JSON.parse(fs.readFileSync(path.join(root, 'package.json'), 'utf8'));
  if (metadata.name !== 'react-native-tvos' || metadata.version !== '0.87.1-0') {
    throw new Error('React profiling supports react-native-tvos 0.87.1-0 only');
  }
  const renderer = path.join(root, 'Libraries/Renderer/implementations/ReactFabric-prod.js');
  const profiling = path.join(root, 'Libraries/Renderer/implementations/ReactFabric-profiling.js');
  if (!fs.existsSync(profiling)) throw new Error('React profiling renderer is unavailable');
  const previous = config.resolver?.resolveRequest;
  return {
    ...config,
    resolver: {
      ...config.resolver,
      resolveRequest(context, moduleName, platform) {
        const resolved = previous
          ? previous(context, moduleName, platform)
          : context.resolveRequest(context, moduleName, platform);
        if (resolved.type === 'sourceFile' && path.resolve(resolved.filePath) === renderer) {
          return { ...resolved, filePath: profiling };
        }
        return resolved;
      },
    },
  };
}
module.exports = { withReactProfiling };
