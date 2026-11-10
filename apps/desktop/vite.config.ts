import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';
import { tokensPlugin } from './src/theme/tokensPlugin.ts';

// Tauri loads `devUrl` from tauri.conf.json, so the port must not drift.
const DEV_PORT = 5173;

export default defineConfig({
  plugins: [react(), tokensPlugin()],
  // Keep Rust compiler output visible when run under `tauri dev`.
  clearScreen: false,
  server: {
    port: DEV_PORT,
    strictPort: true,
    watch: { ignored: ['**/src-tauri/**'] },
  },
  test: {
    environment: 'jsdom',
    include: ['tests/**/*.test.{ts,tsx}'],
    setupFiles: ['./tests/support/setup.ts'],
    // Tests import the app as `@/<module>`; app code itself never uses the alias.
    alias: { '@': fileURLToPath(new URL('./src', import.meta.url)) },
  },
});
