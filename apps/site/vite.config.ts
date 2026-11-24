import { fileURLToPath } from 'node:url';
import react from '@vitejs/plugin-react';
import { defineConfig } from 'vitest/config';
import { tokensPlugin } from '../desktop/src/theme/tokensPlugin.ts';

const page = (name: string) => fileURLToPath(new URL(name, import.meta.url));

// Relative asset URLs: works at carrova.byteveda.org and on any preview path.
export default defineConfig({
  base: './',
  // The app's token stylesheet, for the live demos (live.html) that run the app's own screens.
  plugins: [react(), tokensPlugin()],
  // The app's components import React from their own folder: one copy for both.
  resolve: { dedupe: ['react', 'react-dom'] },
  build: { rolldownOptions: { input: { main: page('index.html'), live: page('live.html') } } },
  test: { include: ['tests/**/*.test.ts'] },
});
