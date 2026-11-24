import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';

// Relative asset URLs: works at carrova.byteveda.org and on any preview path.
export default defineConfig({
  base: './',
  plugins: [react()],
  test: { include: ['tests/**/*.test.ts'] },
});
