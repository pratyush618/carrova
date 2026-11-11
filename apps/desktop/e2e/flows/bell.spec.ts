import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake starts with a request from Ana's laptop, a finished move and an
// older update notice; two of them are new.
test('the bell opens over the page it is on, and a request can be reviewed', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page);
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  const onSettings = page.getByRole('button', { name: 'Check now' });
  await expect(onSettings).toBeVisible();
  const url = page.url();

  await page.getByRole('button', { name: 'Notifications, 2 new' }).click();
  const list = page.getByRole('region', { name: 'Notifications' });
  await expect(list.getByText("Ana's laptop")).toBeVisible();
  await expect(onSettings).toBeVisible();
  expect(page.url()).toBe(url);
  await expectAccessible(page, 'bell');

  await page.keyboard.press('Escape');
  await expect(list).toBeHidden();

  await page.getByRole('button', { name: 'Notifications, 2 new' }).click();
  await list.getByRole('button', { name: 'Review' }).click();
  await expect(list).toBeHidden();
  await expect(page.getByRole('textbox', { name: 'Pairing code' })).toBeVisible();
  expect(errors).toEqual([]);
});
