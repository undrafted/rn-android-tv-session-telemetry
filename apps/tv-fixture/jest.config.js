const path = require('node:path');

// Test the same pinned TV runtime Metro uses, rather than the hoisted mainline RN copy.
const reactNativeRoot = path.dirname(
  require.resolve('react-native/package.json'),
);
module.exports = {
  preset: '@react-native/jest-preset',
  moduleNameMapper: {
    '^react-native/setup-env$': `${reactNativeRoot}/src/setup-env.js`,
    '^react-native($|/.*)': `${reactNativeRoot}/$1`,
  },
};
