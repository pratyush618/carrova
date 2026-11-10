import { expect, test } from '@playwright/test';

test('production build opens on the welcome screen without console errors', async ({ page }) => {
  const errors: string[] = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  page.on('pageerror', (err) => errors.push(err.message));

  await page.goto('/');
  await expect(
    page.getByRole('heading', { level: 1, name: 'Bring your setup to a new computer.' }),
  ).toBeVisible();
  expect(errors).toEqual([]);
});

test('production build has no screen gallery', async ({ page }) => {
  await page.goto('/#gallery/home');
  await expect(
    page.getByRole('heading', { level: 1, name: 'Bring your setup to a new computer.' }),
  ).toBeVisible();
  await expect(page.getByTestId('gallery-frame')).toHaveCount(0);
});

test('bundles its fonts locally', async ({ page }) => {
  const remote: string[] = [];
  page.on('request', (req) => {
    if (!req.url().startsWith('http://localhost')) remote.push(req.url());
  });
  await page.goto('/');
  // The first screen renders once the engine answered; fonts load for laid-out text.
  await expect(
    page.getByRole('heading', { level: 1, name: 'Bring your setup to a new computer.' }),
  ).toBeVisible();
  await page.evaluate(() => document.fonts.ready);
  const loaded = await page.evaluate(() =>
    [...document.fonts].filter((f) => f.status === 'loaded').map((f) => f.family),
  );
  expect(loaded.some((family) => family.includes('Atkinson Hyperlegible Next'))).toBe(true);
  expect(remote).toEqual([]);
});
