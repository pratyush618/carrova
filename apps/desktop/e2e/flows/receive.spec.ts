import { expect, test, type Page } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

async function toReview(page: Page): Promise<void> {
  await page.getByRole('button', { name: /^Receive/ }).click();
  const code = page.getByRole('textbox', { name: 'Pairing code' });
  await code.pressSequentially('7-aci');
  await code.press('Tab');
  await code.pressSequentially('lem');
  await code.press('Tab');
  await expect(code).toHaveValue('7-acid-lemon');
  await expectAccessible(page, 'recv-code');
  await code.press('Enter');
  const approve = page.getByRole('button', { name: 'Approve and move' });
  await expect(approve).toBeVisible();
  // Sensitive items start unconfirmed: the user brings each one.
  const open = page.getByRole('radio', { name: 'Bring them', checked: false });
  while ((await open.count()) > 0) await open.first().click();
  await expect(approve).toBeEnabled();
}

test('receive: code → review (with a per-file rule) → progress → arrived', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await toReview(page);
  await expectAccessible(page, 'recv-review');

  await page.getByRole('button', { name: /See every change/ }).click();
  const sheet = page.getByRole('dialog', { name: 'Every change' });
  await expectAccessible(page, 'review-details');
  const rule = sheet.getByRole('combobox', { name: /What happens to/ }).first();
  await rule.selectOption('keep-both');
  await expect(sheet.getByText('Exists here. Both copies are kept.').first()).toBeVisible();
  await sheet.getByRole('button', { name: 'Done' }).click();

  await page.getByRole('button', { name: 'Approve and move' }).click();
  await expect(page.getByRole('progressbar', { name: 'Overall progress' })).toBeVisible();
  await expectAccessible(page, 'recv-progress');
  await expect(page.getByText('Everything arrived.')).toBeVisible({ timeout: 15_000 });
  await expectAccessible(page, 'recv-done');
  expect(errors).toEqual([]);
});

test('receive: a full disk pauses the transfer, then it picks up again', async ({ page }) => {
  await openApp(page, 'disk-full');
  await toReview(page);
  await page.getByRole('button', { name: 'Approve and move' }).click();
  await expect(page.getByRole('heading', { name: 'Paused: this disk is full' })).toBeVisible();
  await expect(page.getByRole('button', { name: 'Cancel' })).toBeVisible();
  await expectAccessible(page, 'recv-paused');
  await expect(page.getByText('Everything arrived.')).toBeVisible({ timeout: 15_000 });
});

test('undo: arrived → preview → undone', async ({ page }) => {
  await openApp(page);
  await toReview(page);
  await page.getByRole('button', { name: 'Approve and move' }).click();
  await page.getByRole('button', { name: 'Undo this move' }).click({ timeout: 15_000 });
  await expect(page.getByRole('dialog', { name: 'Undo this move?' })).toBeVisible();
  await expectAccessible(page, 'undo-confirm');
  await page.getByRole('button', { name: 'Undo the move' }).click();
  await expect(page.getByText('Undone. This computer is back to how it was.')).toBeVisible();
  await expectAccessible(page, 'undo-result');
});
