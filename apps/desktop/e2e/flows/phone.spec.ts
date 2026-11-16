import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible } from './support';

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
