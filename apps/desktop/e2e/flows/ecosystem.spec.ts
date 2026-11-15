import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake links "Pixel 8" a moment after the invite shows (nothing to scan).
test('pair a phone then send clipboard (fake port)', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Nearby' })
    .click();
  await page.getByRole('button', { name: 'Add a phone' }).click();

  const pairing = page.getByRole('dialog', { name: 'Add a phone' });
  await expect(pairing.getByRole('img', { name: 'Pairing QR code' })).toBeVisible();
  await expect(pairing.getByRole('img', { name: /4 maple orbit/ })).toBeVisible();
  await expectAccessible(page, 'add a phone');

  const linked = page.getByRole('dialog', { name: 'Pixel 8 is linked' });
  await expect(linked).toBeVisible();
  await linked.getByRole('button', { name: 'Choose what it can do' }).click();

  await expect(page.getByRole('heading', { level: 1, name: 'Pixel 8' })).toBeVisible();
  await expect(page.getByRole('switch', { name: 'Shared clipboard' })).toBeChecked();
  await expectAccessible(page, 'device page');
  await page.getByRole('button', { name: 'Send clipboard' }).click();
  await expect(page.getByText('Clipboard sent to Pixel 8')).toBeVisible();

  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Nearby' })
    .click();
  await page.getByRole('radio', { name: 'List' }).click();
  await expect(page.getByRole('button', { name: 'Open: Pixel 8' })).toBeVisible();
  expect(errors).toEqual([]);
});
