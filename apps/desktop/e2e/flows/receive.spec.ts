import { expect, test, type Page } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

/** Pairs and opens the review, nothing answered yet. */
async function pair(page: Page): Promise<void> {
  await page.getByRole('button', { name: /^Receive/ }).click();
  const code = page.getByRole('textbox', { name: 'Pairing code' });
  await code.pressSequentially('7-aci');
  await code.press('Tab');
  await code.pressSequentially('lem');
  await code.press('Tab');
  await expect(code).toHaveValue('7-acid-lemon');
  await expectAccessible(page, 'recv-code');
  await code.press('Enter');
  await expect(page.getByRole('button', { name: 'Approve and move' })).toBeVisible();
}

/** Brings every sensitive item still unanswered; `ready`: nothing else is asked. */
async function bringAll(page: Page, ready = true): Promise<void> {
  // Sensitive items start unconfirmed: the user brings each one.
  const open = page.getByRole('radio', { name: 'Bring them', checked: false });
  while ((await open.count()) > 0) await open.first().click();
  if (ready) await expect(page.getByRole('button', { name: 'Approve and move' })).toBeEnabled();
}

/** Pairs, opens the review and brings every sensitive item; `ready`: nothing else is asked. */
async function toReview(page: Page, ready = true): Promise<void> {
  await pair(page);
  await bringAll(page, ready);
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
  await expect(
    page.getByRole('heading', { name: 'Paused — the disk at /home is full' }),
  ).toBeVisible();
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
  await page.getByRole('button', { name: 'Undo move' }).click();
  await expect(page.getByText('Undone. This computer is back to how it was.')).toBeVisible();
  await expectAccessible(page, 'undo-result');
});

test('receive from a Mac: where things land, and an override behind "I understand"', async ({
  page,
}) => {
  await openApp(page, 'cross-os');
  await toReview(page, false);
  await expect(page.getByText('macOS paths become Linux paths')).toBeVisible();

  await page.getByRole('button', { name: 'See where' }).click();
  const sheet = page.getByRole('dialog', { name: 'Where things land on Linux' });
  await expect(sheet.getByText('/home/ana/.config/nvim').first()).toBeVisible();
  await expect(sheet.getByText('1 more keeps the same place')).toBeVisible();
  await expectAccessible(page, 'mapping-sheet');
  await sheet.getByRole('button', { name: 'Done' }).click();

  await page.getByRole('button', { name: /^Override… / }).click();
  const dialog = page.getByRole('alertdialog', { name: /anyway\?$/ });
  const go = dialog.getByRole('button', { name: 'Bring anyway' });
  await expect(go).toBeDisabled();
  await expectAccessible(page, 'cross-os-override');
  await dialog.getByText(/^I understand/).click();
  await go.click();
  await expect(page.getByText('Bringing it anyway')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Approve and move' })).toBeEnabled();
});

test('receive with a plugin: this computer asks, allowed for this move only', async ({ page }) => {
  await openApp(page, 'plugin');
  await toReview(page, false);
  await expect(page.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
  await page.getByRole('button', { name: 'Review… (path-rewrite plugin)' }).click();
  const dialog = page.getByRole('dialog', { name: 'Allow “path-rewrite”?' });
  await expect(dialog.getByText('It cannot use the network.')).toBeVisible();
  await expect(dialog.getByRole('checkbox')).toHaveCount(0);
  await expectAccessible(page, 'plugin-consent');
  await dialog.getByRole('button', { name: 'Allow for this move' }).click();
  await expect(page.getByText('Allowed for this move')).toBeVisible();
  await expect(page.getByRole('button', { name: 'Approve and move' })).toBeEnabled();
});

test('the "This move" rail folds into a header button below 960 px', async ({ page }) => {
  await openApp(page);
  await toReview(page);
  await expect(page.getByRole('complementary', { name: 'This move' })).toBeVisible();
  await page.setViewportSize({ width: 900, height: 800 });
  await expect(page.getByRole('complementary', { name: 'This move' })).toHaveCount(0);
  await page.getByRole('button', { name: 'This move' }).click();
  const sheet = page.getByRole('dialog', { name: 'This move' });
  await expect(sheet.getByText('ana-desktop')).toBeVisible();
  await expectAccessible(page, 'rail-sheet');
  await page.keyboard.press('Escape');
  await expect(sheet).toHaveCount(0);
});

test('receive Manual picks: accept all safe picks then approve', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page, 'picks');
  await pair(page);
  const accept = page.getByRole('button', { name: 'Accept all 2 files' });
  await expect(accept).toBeVisible();
  await expectAccessible(page, 'recv-review-picks');
  await accept.click();
  await expect(page.getByText('Accepted')).toBeVisible();
  await expect(page.getByRole('radio', { name: 'Bring them', checked: true })).toHaveCount(2);
  // The SSH key pick and the recipe secrets still ask one by one.
  await expect(page.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
  await bringAll(page);

  await page.getByRole('button', { name: 'Approve and move' }).click();
  await expect(page.getByText('Everything arrived.')).toBeVisible({ timeout: 15_000 });
  expect(errors).toEqual([]);
});
