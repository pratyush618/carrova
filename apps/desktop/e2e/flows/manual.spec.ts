import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake dialog host (`?fake=…`) answers Add folder with ~/projects/site and
// Add files with ~/.ssh/id_ed25519 and /etc/hosts.
test('send in Manual mode: pick a folder and a key, refuse /etc/hosts, then pair', async ({
  page,
}) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await page.getByRole('button', { name: /^Send from this computer/ }).click();
  await expect(page.getByRole('heading', { level: 1, name: "Here's what we found" })).toBeVisible();

  await page.getByRole('switch', { name: /^Manual/ }).click();
  await expect(
    page.getByRole('heading', { level: 1, name: 'Pick files and folders' }),
  ).toBeVisible();
  await expectAccessible(page, 'manual-empty');

  const picks = page.getByRole('list', { name: 'Your picks' });
  await page.getByRole('button', { name: 'Add folder' }).click();
  await expect(picks.getByText('~/projects/site')).toBeVisible();

  await page.getByRole('button', { name: 'Add files' }).click();
  await expect(
    page.getByRole('list', { name: "Can't add" }).getByText(/outside your home and app folders/),
  ).toBeVisible();
  const key = page.getByRole('checkbox', { name: 'Bring ~/.ssh/id_ed25519' });
  await expect(key).not.toBeChecked();
  await expectAccessible(page, 'manual-picks');

  await page.getByRole('button', { name: 'Include ~/.ssh/id_ed25519…' }).click();
  const dialog = page.getByRole('alertdialog', { name: 'Bring ~/.ssh/id_ed25519?' });
  await expect(dialog).toBeVisible();
  await expectAccessible(page, 'manual-key-confirm');
  await dialog.getByRole('button', { name: 'Bring them' }).click();
  await expect(key).toBeChecked();

  await page.getByRole('switch', { name: /^Manual/ }).click();
  await expect(page.getByText('Manual · 2')).toBeVisible();
  await page.getByRole('switch', { name: /^Manual/ }).click();
  await expect(picks.getByText('~/projects/site')).toBeVisible();

  await page.getByRole('button', { name: 'Get pairing code' }).click();
  await expect(page.getByRole('img', { name: 'Pairing code: 7 acid lemon' })).toBeVisible();
  expect(errors).toEqual([]);
});
