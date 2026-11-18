import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake's linked "Pixel 8" has Messages off here; turning it on adds the
// Messages page, over the fake's canned conversations.
test('turn on Messages for a phone, then read and send a text (fake port)', async ({ page }) => {
  const errors = consoleErrors(page);
  await openApp(page, 'linked');
  const nav = page.getByRole('navigation', { name: 'Main' });
  await expect(nav.getByRole('button', { name: 'Messages' })).toHaveCount(0);
  await nav.getByRole('button', { name: 'Nearby' }).click();
  await page.getByRole('radio', { name: 'List' }).click();
  await page.getByRole('button', { name: 'Open: Pixel 8' }).click();
  await page.getByRole('switch', { name: 'Messages' }).click();

  await nav.getByRole('button', { name: 'Messages' }).click();
  const list = page.getByRole('navigation', { name: 'Conversations' });
  await expect(list.getByRole('button', { name: /Sam Rivera/ })).toHaveAttribute(
    'aria-current',
    'true',
  );
  await expect(page.getByRole('heading', { name: 'Sam Rivera' })).toBeVisible();
  await expect(page.getByText('On my way')).toBeVisible();
  await expectAccessible(page, 'messages');

  await page.getByRole('textbox', { name: 'Text message' }).fill('Leaving in 10');
  await page.getByRole('button', { name: 'Send' }).click();
  await expect(page.getByText('Leaving in 10')).toHaveCount(0);

  await page.getByRole('button', { name: 'New message' }).click();
  await page.getByRole('textbox', { name: 'To' }).fill('Sam');
  await page
    .getByRole('button', { name: /Sam Rivera/ })
    .last()
    .click();
  await expect(page.getByRole('heading', { name: 'Sam Rivera' })).toBeVisible();
  expect(errors).toEqual([]);
});

// The pop-up window loads the same bundle with its target in the query.
test('reply to a phone notification from its pop-up (fake port)', async ({ page }) => {
  const errors = consoleErrors(page);
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto('/?fake=linked&popup=note&device=p8&key=n1&name=Pixel+8');
  await expect(page.getByText('Chat · Pixel 8')).toBeVisible();
  await expect(page.getByText('Are you still coming tonight?')).toBeVisible();
  await expectAccessible(page, 'notification pop-up');
  await page.getByRole('textbox', { name: 'Reply' }).fill('Yes, leaving in 10');
  await page.getByRole('button', { name: 'Send' }).click();
  await expect(page.getByText('Are you still coming tonight?')).toHaveCount(0);
  expect(errors).toEqual([]);
});
