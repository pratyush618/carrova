// Captures the site's product shots from the desktop app's screen gallery and
// its in-memory fake, in light and dark, as WebP at two widths.
// Run with `just site-shots` (starts the desktop dev server itself).
import { spawn } from 'node:child_process';
import { mkdir, writeFile } from 'node:fs/promises';
import { dirname, join } from 'node:path';
import { createRequire } from 'node:module';
import { fileURLToPath } from 'node:url';

const here = dirname(fileURLToPath(import.meta.url));
const OUT = join(here, '../src/assets/shots');
const DESKTOP = join(here, '../../desktop');
// Borrow the desktop app's Playwright rather than pin a second copy.
const { chromium } = createRequire(join(DESKTOP, 'package.json'))('@playwright/test');
const PORT = 5199;
const BASE = `http://localhost:${String(PORT)}`;

/** Gallery screens: name → `#gallery/<screen>:<state>`. */
const GALLERY = {
  review: 'recv-review:ready',
  decide: 'recv-review:decide',
  send: 'send:found',
  code: 'pair:waiting',
  linked: 'pair:linked',
  done: 'recv-done:done',
  nearby: 'nearby:map',
  undo: 'undo-confirm',
  apps: 'apps:sensitive',
};

const WINDOW = { width: 1100, height: 720 };

async function waitForServer() {
  for (let i = 0; i < 120; i++) {
    try {
      if ((await fetch(BASE)).ok) return;
    } catch {
      // not up yet
    }
    await new Promise((r) => setTimeout(r, 500));
  }
  throw new Error(`desktop dev server did not start on ${BASE}`);
}

/** Encodes a PNG as WebP at each width, in the browser (no image tooling needed). */
async function saveWebp(encoder, png, name, widths) {
  const src = `data:image/png;base64,${png.toString('base64')}`;
  for (const w of widths) {
    const dataUrl = await encoder.evaluate(
      async ({ src, w }) => {
        const img = new Image();
        img.src = src;
        await img.decode();
        const c = document.createElement('canvas');
        c.width = w;
        c.height = Math.round((img.height * w) / img.width);
        const g = c.getContext('2d');
        g.imageSmoothingQuality = 'high';
        g.drawImage(img, 0, 0, c.width, c.height);
        return c.toDataURL('image/webp', 0.86);
      },
      { src, w },
    );
    const file = join(OUT, `${name}-${String(w)}.webp`);
    await writeFile(file, Buffer.from(dataUrl.split(',')[1], 'base64'));
    console.log(file);
  }
}

const server = spawn('pnpm', ['exec', 'vite', '--port', String(PORT), '--strictPort'], {
  cwd: DESKTOP,
  stdio: 'ignore',
});
try {
  await waitForServer();
  await mkdir(OUT, { recursive: true });
  const browser = await chromium.launch({ channel: 'chrome' });
  const encoder = await browser.newPage();

  for (const scheme of ['light', 'dark']) {
    const page = await browser.newPage({
      viewport: { width: 1200, height: 900 },
      deviceScaleFactor: 2,
    });
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    for (const [name, target] of Object.entries(GALLERY)) {
      await page.goto(`${BASE}/#gallery/${target}`);
      const frame = page.locator('[data-testid=gallery-frame]').first();
      await frame.waitFor();
      await page.waitForTimeout(500);
      const png = await frame.screenshot({ animations: 'disabled' });
      await saveWebp(encoder, png, `${name}.${scheme}`, [1200, 2200]);
    }
    await page.close();

    // A linked Pixel 8 over the fake, as the laptop sees it.
    const app = await browser.newPage({ viewport: WINDOW, deviceScaleFactor: 2 });
    await app.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    await app.goto(`${BASE}/?fake=linked`);
    const nav = app.getByRole('navigation', { name: 'Main' });
    await nav.getByRole('button', { name: 'Nearby' }).click();
    await app.getByRole('radio', { name: 'List' }).click();
    await app.getByRole('button', { name: 'Open: Pixel 8' }).click();
    await app.waitForTimeout(500);
    // The sidebar is cropped off: the fake's dev recipe folder shows a warning there.
    const main = { x: 212, y: 0, width: WINDOW.width - 212, height: WINDOW.height };
    await saveWebp(encoder, await app.screenshot({ clip: main }), `device.${scheme}`, [900, 1776]);
    await app.getByRole('switch', { name: 'Messages' }).click();
    await nav.getByRole('button', { name: 'Messages' }).click();
    await app.getByRole('heading', { name: 'Sam Rivera' }).waitFor();
    await saveWebp(
      encoder,
      await app.screenshot({ clip: main }),
      `messages.${scheme}`,
      [900, 1776],
    );
    await app.close();

    // The notification pop-up window.
    const note = await browser.newPage({
      viewport: { width: 380, height: 200 },
      deviceScaleFactor: 2,
    });
    await note.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });
    await note.goto(`${BASE}/?fake=linked&popup=note&device=p8&key=n1&name=Pixel+8`);
    await note.getByText('Sam Rivera').waitFor();
    await saveWebp(encoder, await note.screenshot(), `note.${scheme}`, [760]);
    await note.close();
  }
  await browser.close();
} finally {
  server.kill();
}
