import { expect, test } from '@playwright/test';
import { HOME_TITLE, expectAccessible } from './flows/support';

test('production build opens on Home without console errors', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  page.on('pageerror', (err) => errors.push(err.message));

  await page.goto('/');
  await expect(page.getByRole('heading', { level: 1, name: HOME_TITLE })).toBeVisible();
  expect(errors).toEqual([]);
});

test('first run asks once about error reports, and S starts sending', async ({ page }) => {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/');
  const question = page.getByRole('heading', { name: 'Send error reports if Carrova crashes?' });
  await expect(question).toBeVisible();
  await expectAccessible(page, 'home');
  await page.getByRole('button', { name: 'No thanks' }).click();
  await expect(question).toHaveCount(0);

  await page.keyboard.press('s');
  await expect(page.getByRole('heading', { level: 1, name: HOME_TITLE })).toHaveCount(0);
});

test('History opens the newest move beside the list', async ({ page }) => {
  await page.goto('/');
  await page.getByRole('button', { name: 'History' }).click();
  await expect(page.getByRole('complementary', { name: 'This move' })).toBeVisible();
  await expectAccessible(page, 'history');
});

test('production build has no screen gallery', async ({ page }) => {
  await page.goto('/#gallery/home');
  await expect(page.getByRole('heading', { level: 1, name: HOME_TITLE })).toBeVisible();
  await expect(page.getByTestId('gallery-frame')).toHaveCount(0);
});

test('bundles its fonts locally', async ({ page }) => {
  const remote: string[] = [];
  page.on('request', (req) => {
    if (!req.url().startsWith('http://localhost')) remote.push(req.url());
  });
  await page.goto('/');
  // The first screen renders once the engine answered; fonts load for laid-out text.
  await expect(page.getByRole('heading', { level: 1, name: HOME_TITLE })).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  const loaded = await page.evaluate(() =>
    [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family),
  );
  expect(loaded.some((family) => family.includes('Atkinson Hyperlegible Next'))).toBe(true);
  expect(remote).toEqual([]);
});
