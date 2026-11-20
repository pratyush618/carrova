import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake `known-send` network: Desk PC (linked) opens Review, Ana's laptop
// declines, new-mbp needs a code. `move-request`: Desk PC asks this computer.

test('send to a trusted computer: choose → waiting → reviewing → delivered, no code', async ({
  page,
}) => {
  const errors = consoleErrors(page);
  await openApp(page, 'known-send');
  await page.getByRole('button', { name: /^Send from this computer/ }).click();
  await page.getByRole('button', { name: 'Choose a computer' }).click();

  const sheet = page.getByRole('dialog', { name: 'Send your setup to…' });
  await expect(sheet.getByRole('button', { name: 'Send to Desk PC' })).toBeVisible();
  await expectAccessible(page, 'sendto-choose');
  await sheet.getByRole('button', { name: 'Send to Desk PC' }).click();

  await expect(page.getByRole('dialog', { name: 'Waiting for Desk PC' })).toBeVisible();
  await expectAccessible(page, 'sendwait-waiting');
  const reviewing = page.getByRole('dialog', { name: 'Desk PC is reviewing' });
  await expect(reviewing).toBeVisible({ timeout: 10_000 });
  await reviewing.getByRole('button', { name: 'Close' }).click();

  await expect(page.getByText('Your setup is on the new computer.')).toBeVisible({
    timeout: 15_000,
  });
  await expect(page.getByRole('img', { name: /^Pairing code/ })).toHaveCount(0);
  expect(errors).toEqual([]);
});

test('a declined request goes back to what moves', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page, 'known-send');
  await page.getByRole('button', { name: /^Send from this computer/ }).click();
  await page.getByRole('button', { name: 'Choose a computer' }).click();
  await page.getByRole('button', { name: "Send to Ana's laptop" }).click();

  const declined = page.getByRole('alertdialog', { name: "Ana's laptop declined" });
  await expect(declined).toBeVisible({ timeout: 10_000 });
  await expectAccessible(page, 'sendwait-declined');
  await declined.getByRole('button', { name: 'Change what moves' }).click();
  await expect(page.getByRole('heading', { level: 1, name: "Here's what we found" })).toBeVisible();
  expect(errors).toEqual([]);
});

// From Review on it is the receive flow `receive.spec.ts` covers.
test('a move request on Home opens the receive review', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page, 'move-request');
  await expect(page.getByText('Desk PC wants to send you its setup')).toBeVisible();
  await expectAccessible(page, 'sendreq-open');
  await page.getByRole('button', { name: 'Review' }).click();

  await expect(
    page.getByRole('heading', { level: 1, name: "Here's what will change" }),
  ).toBeVisible();
  expect(errors).toEqual([]);
});
