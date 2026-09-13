// @react-native/eslint-config's legacy .eslintrc format doesn't work with the ESLint version
// this app was scaffolded with (ESLint 8's legacy config resolution reports every file as
// ignored against this exact @react-native/eslint-config@0.87.1 release — an upstream
// compatibility gap, not something specific to this project). Using its flat-config export
// instead, on the same ESLint version the rest of the monorepo already uses.
const reactNativeFlatConfig = require('@react-native/eslint-config/flat');

module.exports = [
  {
    ignores: ['node_modules/**', 'android/**'],
  },
  ...reactNativeFlatConfig,
  {
    // Metro/Babel/Jest config files run under plain Node/CommonJS, not the RN/Hermes runtime.
    files: ['*.config.js', 'babel-plugin-*.js', '.prettierrc.js'],
    languageOptions: {
      sourceType: 'commonjs',
      globals: {
        module: 'writable',
        require: 'readonly',
        __dirname: 'readonly',
        process: 'readonly',
      },
    },
  },
  {
    // Injected by babel-plugin-rnst-profiling-flag.js at bundle time; see babel.config.js.
    files: ['index.js'],
    languageOptions: {
      globals: {
        __RN_SESSION_TELEMETRY_ENABLED__: 'readonly',
        __RN_SESSION_TELEMETRY_BENCHMARK__: 'readonly',
      },
    },
  },
];
