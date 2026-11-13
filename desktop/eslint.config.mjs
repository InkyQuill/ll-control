import js from '@eslint/js';
import tseslint from 'typescript-eslint';
import svelte from 'eslint-plugin-svelte';
import local from './scripts/eslint-local-rules.mjs';

export default [
  {
    ignores: [
      'node_modules',
      'build',
      '.svelte-kit',
      'test-results',
      'src-tauri/gen',
      'playwright-report',
      'src/lib/rpc-contract.ts',
    ],
  },
  js.configs.recommended,
  ...tseslint.configs.recommended,
  ...svelte.configs['flat/recommended'],
  {
    files: ['**/*.{js,mjs,ts,svelte}'],
    languageOptions: {
      parserOptions: { parser: tseslint.parser, extraFileExtensions: ['.svelte'] },
    },
    plugins: { local },
    rules: {
      'no-undef': 'off',
      'no-control-regex': 'off',
    },
  },
  {
    files: ['src/**/*.{ts,svelte}'],
    plugins: { local },
    rules: {
      'local/no-async-effect': 'error',
      'local/no-literal-error': 'error',
    },
  },
  {
    files: ['tests/**/*.ts'],
    rules: {
      '@typescript-eslint/no-unused-vars': 'off',
      '@typescript-eslint/no-explicit-any': 'off',
    },
  },
];
