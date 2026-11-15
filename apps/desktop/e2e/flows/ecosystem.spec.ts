import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake links "Pixel 8" a moment after the invite shows (nothing to scan).
test('add a device then send clipboard (fake port)', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Nearby' })
    .click();
  await page.getByRole('button', { name: 'Add a device' }).click();

  const pairing = page.getByRole('dialog', { name: 'Add a device' });
  await expect(pairing.getByRole('img', { name: 'Pairing QR code' })).toBeVisible();
  await expect(pairing.getByRole('img', { name: /4 maple orbit/ })).toBeVisible();
  await expect(
    pairing.getByText('On a computer, choose Link a computer and type this code'),
  ).toBeVisible();
  await expectAccessible(page, 'add a device');

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

test('switch the clipboard settings (fake port)', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Settings' })
    .click();
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: 'Clipboard' })
    .click();

  const note = page.getByRole('switch', { name: 'Show a note when something arrives' });
  await expect(page.getByRole('switch', { name: 'Share the clipboard' })).toBeChecked();
  await expect(note).not.toBeChecked();
  await note.click();
  await expect(note).toBeChecked();
  await expectAccessible(page, 'clipboard settings');

  // Saved: still on after leaving the section and coming back.
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: 'General' })
    .click();
  await page
    .getByRole('navigation', { name: 'Settings sections' })
    .getByRole('button', { name: 'Clipboard' })
    .click();
  await expect(note).toBeChecked();
  expect(errors).toEqual([]);
});
