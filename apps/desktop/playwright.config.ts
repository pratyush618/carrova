import { defineConfig, devices } from '@playwright/test';

// Two builds of the same sources, both production-optimised: a `--mode e2e`
// bundle (→ dist-e2e/) that is the app Tauri embeds plus the in-memory fake
// port (`?fake=<scenario>`; `pnpm build` leaves the fake out entirely), and a
// `--mode gallery` bundle that keeps the dev screen gallery.
const APP_PORT = 4173;
const GALLERY_PORT = 4174;
const url = (port: number): string => `http://localhost:${String(port)}`;

export default defineConfig({
  testDir: './e2e',
  forbidOnly: Boolean(process.env['CI']),
  retries: process.env['CI'] ? 2 : 0,
  // Shared laptop: keep browser parallelism modest.
  workers: 3,
  reporter: 'list',
  use: { trace: 'on-first-retry' },
  // Tauri on Linux renders with WebKitGTK; Chromium is the fast default here.
  projects: [
    {
      name: 'app',
      testMatch: /smoke\.spec\.ts/,
      use: { ...devices['Desktop Chrome'], baseURL: url(APP_PORT) },
    },
    {
      // Every flow end to end over the in-memory fake port (`?fake=<scenario>`).
      name: 'flows',
      testMatch: /flows\/.*\.spec\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        baseURL: url(APP_PORT),
        viewport: { width: 1100, height: 800 },
      },
    },
    {
      name: 'gallery',
      testMatch: /gallery\.spec\.ts/,
      use: {
        ...devices['Desktop Chrome'],
        baseURL: url(GALLERY_PORT),
        viewport: { width: 1200, height: 900 },
      },
    },
  ],
  webServer: [
    {
      command: `pnpm exec vite preview --outDir dist-e2e --port ${String(APP_PORT)} --strictPort`,
      url: url(APP_PORT),
      reuseExistingServer: !process.env['CI'],
    },
    {
      command: `pnpm exec vite build --mode gallery --outDir dist-gallery --emptyOutDir && pnpm exec vite preview --outDir dist-gallery --port ${String(GALLERY_PORT)} --strictPort`,
      url: url(GALLERY_PORT),
      reuseExistingServer: !process.env['CI'],
      timeout: 120_000,
    },
  ],
});
