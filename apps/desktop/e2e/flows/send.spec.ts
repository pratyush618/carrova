import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, HOME_TITLE, openApp } from './support';

test('send: found → code → live → delivered', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await page.getByRole('button', { name: /^Send from this computer/ }).click();
  await expect(page.getByRole('heading', { level: 1, name: "Here's what we found" })).toBeVisible();
  await expectAccessible(page, 'send-found');

  await page.getByRole('button', { name: 'Include SSH key pairs (private keys)…' }).click();
  const keys = page.getByRole('alertdialog', { name: 'Bring SSH key pairs (private keys)?' });
  await expect(keys).toBeVisible();
  await expectAccessible(page, 'key-confirm');
  await keys.getByRole('button', { name: 'Leave behind' }).click();
  await expect(keys).toBeHidden();

  await page.getByRole('button', { name: 'Customize' }).click();
  await expect(page.getByRole('dialog', { name: 'Customize what moves' })).toBeVisible();
  await expectAccessible(page, 'customize');
  await page.keyboard.press('Escape');

  await page.getByRole('button', { name: 'Get pairing code' }).click();
  await expect(page.getByRole('img', { name: 'Pairing code: 7 acid lemon' })).toBeVisible();
  await expectAccessible(page, 'send-code');

  await expect(
    page.getByRole('heading', { name: /^Linked\. Only .+ can read this\.$/ }),
  ).toBeVisible();
  await expectAccessible(page, 'send-linked');

  await expect(page.getByText('Your setup is on the new computer.')).toBeVisible({
    timeout: 15_000,
  });
  await expectAccessible(page, 'send-done');
  await page.getByRole('button', { name: 'Done' }).click();
  await expect(page.getByRole('heading', { level: 1, name: HOME_TITLE })).toBeVisible();
  expect(errors).toEqual([]);
});

test('send: a wrong code asks before a new one is shown', async ({ page }) => {
  await openApp(page, 'wrong-code');
  await page.getByRole('button', { name: /^Send from this computer/ }).click();
  await page.getByRole('button', { name: 'Get pairing code' }).click();

  const dialog = page.getByRole('alertdialog', { name: 'Someone typed a wrong code' });
  await expect(dialog).toBeVisible();
  await expectAccessible(page, 'confirm-new-code');
  await dialog.getByRole('button', { name: 'Show a new code' }).click();

  await expect(page.getByRole('img', { name: 'Pairing code: 4 violet harbor' })).toBeVisible();
  await expect(page.getByText('A wrong code was typed, so we made a new one')).toBeVisible();
  await expect(page.getByText('Your setup is on the new computer.')).toBeVisible({
    timeout: 15_000,
  });
});
