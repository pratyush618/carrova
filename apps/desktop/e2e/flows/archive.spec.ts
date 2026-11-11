import { expect, test } from '@playwright/test';
import { expectAccessible, openApp } from './support';

const PASSPHRASE = 'orbit lemon tiger canvas';

test('archive: save with a passphrase, then open it into the review', async ({ page }) => {
  await openApp(page);
  await page.getByRole('button', { name: 'Use an encrypted file instead' }).click();
  await expectAccessible(page, 'file-choose');
  await page.getByRole('button', { name: /Save to a file/ }).click();
  await page.getByRole('button', { name: 'Continue' }).click();

  await page.getByLabel('Passphrase', { exact: true }).fill(PASSPHRASE);
  await page.getByLabel('Type it again').fill(PASSPHRASE);
  await page.getByLabel('Save to').fill('/tmp/setup.carrova');
  await expectAccessible(page, 'save-lock');
  await page.getByRole('button', { name: 'Save file' }).click();
  await expect(page.getByText('Your setup is in one file.')).toBeVisible({ timeout: 15_000 });
  await expectAccessible(page, 'save-done');
  await page.getByRole('button', { name: 'Done' }).click();

  await page.getByRole('button', { name: 'Use an encrypted file instead' }).click();
  await page.getByRole('button', { name: /Open a file/ }).click();
  await page.getByLabel('File', { exact: true }).fill('/tmp/setup.carrova');
  await page.getByLabel('Passphrase').fill('wrong');
  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await expect(
    page.getByText("That passphrase doesn't open this file.", { exact: false }),
  ).toBeVisible();
  await expectAccessible(page, 'open-file-wrong');
  await page.getByLabel('Passphrase').fill(PASSPHRASE);
  await page.getByRole('button', { name: 'Open', exact: true }).click();
  await expect(page.getByRole('button', { name: 'Approve and move' })).toBeVisible();
});

test('history and settings render live data accessibly', async ({ page }) => {
  await openApp(page);
  await page.getByRole('button', { name: 'History', exact: true }).click();
  await expect(page.getByRole('button', { name: 'History', exact: true })).toHaveAttribute(
    'aria-current',
    'page',
  );
  await expectAccessible(page, 'history');
  await page.getByRole('button', { name: /Opened a file/ }).click();
  await expectAccessible(page, 'session-sheet');
  await page.keyboard.press('Escape');

  await page.getByRole('button', { name: 'Apps', exact: true }).click();
  await expect(page.getByRole('list', { name: 'Apps' })).toBeVisible();
  await expectAccessible(page, 'apps');

  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('button', { name: 'Check now' }).click();
  await expect(page.getByText('You have the newest list.')).toBeVisible();
  await expectAccessible(page, 'settings');
});
