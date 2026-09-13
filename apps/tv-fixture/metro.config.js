const { getDefaultConfig, mergeConfig } = require('@react-native/metro-config');
const path = require('path');

// This is an npm workspace, so hoisted dependencies (@babel/runtime included) live in the
// monorepo root's node_modules, not this app's own - Metro's default config only looks in the
// latter unless told about the former.
const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, '../..');

function escapeRegExp(value) {
  return value.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
}

// react-native (aliased to react-native-tvos) ends up physically duplicated on disk in two
// places, and Metro treats same-content-different-path modules as distinct instances - that
// splits singleton registries like ReactNativeViewConfigRegistry across copies (one registers a
// component's view config, another looks it up and finds nothing: "View config getter callback
// ... must be a function"):
//
//  1. @rn-session-telemetry/react-native has its own local install (packages/react-native/
//     node_modules/react-native), separate from this app's, even though it's the same version.
//  2. @react-native-tvos/virtualized-lists (a dependency of react-native-tvos itself) depends on
//     plain "react-native", which npm nests under this app's react-native-tvos install - and
//     Node's own resolution finds that nested copy before it ever reaches the outer one.
//
// Both mechanisms below are needed together: blockList alone made resolution for #2 fall
// through past this app's own copy to a third, unrelated vanilla react-native hoisted at the
// monorepo root (still wrong, just a different wrong copy) - resolveRequest alone caught #1 but
// never saw #2's request at all (it isn't reached through the plain moduleName lookup this hook
// intercepts). blockList hides both duplicates from Metro; resolveRequest then pins the exact
// bare 'react-native' specifier to this app's own copy rather than letting fallback resolution
// pick whichever one happens to be nearest.
const duplicateReactNativePaths = [
  path.join(workspaceRoot, 'packages/react-native/node_modules/react-native'),
  path.join(projectRoot, 'node_modules/react-native/node_modules/react-native'),
];
const canonicalReactNative = path.resolve(projectRoot, 'node_modules/react-native');

/**
 * Metro configuration
 * https://reactnative.dev/docs/metro
 *
 * @type {import('@react-native/metro-config').MetroConfig}
 */
const config = {
  watchFolders: [workspaceRoot],
  resolver: {
    nodeModulesPaths: [
      path.resolve(projectRoot, 'node_modules'),
      path.resolve(workspaceRoot, 'node_modules'),
    ],
    blockList: duplicateReactNativePaths.map(
      (blockedPath) => new RegExp(`^${escapeRegExp(blockedPath)}/.*$`),
    ),
    resolveRequest: (context, moduleName, platform) => {
      if (moduleName === 'react-native') {
        return context.resolveRequest(context, canonicalReactNative, platform);
      }
      return context.resolveRequest(context, moduleName, platform);
    },
  },
};

module.exports = mergeConfig(getDefaultConfig(projectRoot), config);
