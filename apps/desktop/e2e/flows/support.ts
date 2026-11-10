import AxeBuilder from '@axe-core/playwright';
import { expect, type Page } from '@playwright/test';

/** Home's heading once the fake's recorded moves have loaded (it has some, so: returning). */
export const HOME_TITLE = 'Good to see you again.';

/** Opens the production build over the fake port acting out `scenario`, on Home. */
export async function openApp(page: Page, scenario = 'happy'): Promise<void> {
  await page.emulateMedia({ reducedMotion: 'reduce' });
  await page.goto(`/?fake=${scenario}`);
  await expect(page.getByRole('heading', { level: 1, name: HOME_TITLE })).toBeVisible();
}

/** No serious or critical WCAG 2.2 AA violations on what is on screen now. */
export async function expectAccessible(page: Page, step: string): Promise<void> {
  const { violations } = await new AxeBuilder({ page })
    .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
    .analyze();
  const blocking = violations
    .filter((v) => v.impact === 'serious' || v.impact === 'critical')
    .map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);
  expect.soft(blocking, step).toEqual([]);
}

/** Collects console errors and page errors; assert it stays empty at the end. */
export function consoleErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('console', (msg) => {
    if (msg.type() === 'error') errors.push(msg.text());
  });
  page.on('pageerror', (err) => errors.push(err.message));
  return errors;
}
