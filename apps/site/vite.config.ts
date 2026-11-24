import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Served from https://<user>.github.io/carrova/, so assets need the repo prefix.
export default defineConfig({
  base: './',
  plugins: [react()],
  test: { include: ['tests/**/*.test.ts'] },
});
