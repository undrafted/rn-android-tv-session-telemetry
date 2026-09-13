const { getDefaultConfig, mergeConfig } = require('@react-native/metro-config');
const path = require('path');

// This is an npm workspace, so hoisted dependencies (@babel/runtime included) live in the
// monorepo root's node_modules, not this app's own - Metro's default config only looks in the
// latter unless told about the former.
const projectRoot = __dirname;
const workspaceRoot = path.resolve(projectRoot, '../..');

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
  },
};

module.exports = mergeConfig(getDefaultConfig(projectRoot), config);
