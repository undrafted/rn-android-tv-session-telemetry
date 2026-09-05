import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import eslintConfigPrettier from 'eslint-config-prettier';

export default tseslint.config(
  {
    // apps/tv-fixture ships its own eslint setup (@react-native/eslint-config, legacy
    // .eslintrc.js format, its own eslint version) via its own `lint` script — the RN CLI
    // template owns that config, not this repo-wide flat config.
    ignores: [
      '**/dist/**',
      '**/node_modules/**',
      '**/target/**',
      'android/**/build/**',
      'apps/tv-fixture/**',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  eslintConfigPrettier,
  {
    files: ['**/*.{ts,tsx}'],
    rules: {
      '@typescript-eslint/no-unused-vars': [
        'warn',
        { argsIgnorePattern: '^_', varsIgnorePattern: '^_' },
      ],
    },
  },
);
