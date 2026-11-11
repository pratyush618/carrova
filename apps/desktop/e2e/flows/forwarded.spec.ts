import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible } from './support';

// `?fake=forwarded`: the app was opened from the file manager's "Send with
// Carrova" with ~/projects/site and ~/notes.txt.
test('send with Carrova: the app opens on a Manual send with the picks', async ({ page }) => {
  const errors = consoleErrors(page);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?fake=forwarded');
  await expect(
    page.getByRole('heading', { level: 1, name: 'Pick files and folders' }),
  ).toBeVisible();
  const picks = page.getByRole('list', { name: 'Your picks' });
  await expect(picks.getByText('~/projects/site')).toBeVisible();
  await expect(picks.getByText('~/notes.txt')).toBeVisible();
  await expectAccessible(page, 'forwarded-send');

  await page.getByRole('button', { name: 'Get pairing code' }).click();
  await expect(page.getByRole('img', { name: 'Pairing code: 7 acid lemon' })).toBeVisible();
  expect(errors).toEqual([]);
});
