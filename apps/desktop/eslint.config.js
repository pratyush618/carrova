import js from '@eslint/js';
import prettier from 'eslint-config-prettier';
import i18next from 'eslint-plugin-i18next';
import reactHooks from 'eslint-plugin-react-hooks';
import { defineConfig, globalIgnores } from 'eslint/config';
import globals from 'globals';
import tseslint from 'typescript-eslint';

export default defineConfig([
  globalIgnores([
    'dist',
    'dist-gallery',
    'dist-e2e',
    'src-tauri',
    'playwright-report',
    'test-results',
    'src/bindings',
    // Own package with its own tsconfig (WebdriverIO globals): `tsc` checks it.
    'e2e-app',
  ]),
  {
    files: ['**/*.{ts,tsx}'],
    extends: [
      js.configs.recommended,
      tseslint.configs.strictTypeChecked,
      tseslint.configs.stylisticTypeChecked,
      reactHooks.configs.flat.recommended,
    ],
    languageOptions: {
      globals: globals.browser,
      parserOptions: { projectService: true, tsconfigRootDir: import.meta.dirname },
    },
  },
  {
    // Every user-facing string comes from the i18n catalogue: no literal JSX
    // text, and no literal accessible names or placeholders.
    files: ['src/**/*.tsx'],
    ignores: ['src/gallery/**'],
    plugins: { i18next },
    rules: {
      'i18next/no-literal-string': [
        'error',
        {
          mode: 'jsx-only',
          'jsx-attributes': { include: ['aria-label', 'title', 'placeholder', 'alt', 'label'] },
        },
      ],
    },
  },
  {
    files: ['eslint.config.js'],
    extends: [js.configs.recommended],
    languageOptions: { globals: globals.node },
  },
  // Last: turn off stylistic rules that Prettier owns.
  prettier,
]);
