// @ts-check

import eslint from '@eslint/js';
import { defineConfig } from 'eslint/config';
import tseslint from 'typescript-eslint';

export default defineConfig(
  eslint.configs.recommended,
  tseslint.configs.recommended,
  {
    ignores: ["node_modules/**"],
  },
  {
    rules: {
      "@typescript-eslint/no-explicit-any": "off"
    }
  },
);

