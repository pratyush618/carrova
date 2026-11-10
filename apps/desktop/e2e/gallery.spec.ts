import AxeBuilder from '@axe-core/playwright';
import { expect, test, type Page } from '@playwright/test';

type Scheme = 'light' | 'dark';

/** Every `screen:state` the gallery offers, read from its own index and frames. */
async function allTargets(page: Page): Promise<string[]> {
  await page.goto('/#gallery');
  await page.getByRole('button', { name: /Screen index/ }).click();
  const hrefs = await page
    .getByRole('navigation', { name: 'Screen index' })
    .getByRole('link')
    .evaluateAll((links) => links.map((a) => a.getAttribute('href') ?? ''));
  const targets: string[] = [];
  for (const href of hrefs) {
    const id = href.replace('#gallery/', '').split(':')[0] ?? '';
    await page.goto(`/#gallery/${id}`);
    const frame = page.locator(`[data-testid=gallery-frame][data-screen="${id}"]`);
    await expect(frame).toBeVisible();
    const states = ((await frame.getAttribute('data-states')) ?? '').split(' ').filter(Boolean);
    targets.push(...(states.length ? states.map((s) => `${id}:${s}`) : [id]));
  }
  return targets;
}

for (const scheme of ['light', 'dark'] as const satisfies readonly Scheme[]) {
  test(`every screen renders in ${scheme} without console errors or serious a11y violations`, async ({
    page,
  }) => {
    test.setTimeout(240_000);
    const errors: string[] = [];
    page.on('console', (msg) => {
      if (msg.type() === 'error') errors.push(msg.text());
    });
    page.on('pageerror', (err) => errors.push(err.message));
    // Reduced motion: axe measures settled colours, not mid-animation frames.
    await page.emulateMedia({ colorScheme: scheme, reducedMotion: 'reduce' });

    const targets = await allTargets(page);
    expect(targets.length).toBeGreaterThanOrEqual(29);

    for (const target of targets) {
      await test.step(target, async () => {
        const [id = '', state = ''] = target.split(':');
        await page.goto(`/#gallery/${target}`);
        const frame = page.locator(
          `[data-testid=gallery-frame][data-screen="${id}"][data-state="${state}"]`,
        );
        await expect(frame).toBeVisible();
        await expect(frame.locator('h1, h2').first()).toBeVisible();

        const { violations } = await new AxeBuilder({ page })
          .include('[data-testid=gallery-frame]')
          .withTags(['wcag2a', 'wcag2aa', 'wcag21a', 'wcag21aa', 'wcag22aa'])
          .analyze();
        const blocking = violations
          .filter((v) => v.impact === 'serious' || v.impact === 'critical')
          .map((v) => `${v.id}: ${v.nodes.map((n) => n.target.join(' ')).join(', ')}`);
        expect.soft(blocking, `${target} (${scheme})`).toEqual([]);
      });
    }
    expect(errors).toEqual([]);
  });
}

test('the pseudo-locale renders every screen without missing keys', async ({ page }) => {
  test.setTimeout(120_000);
  await page.goto('/#gallery/home');
  await page.getByRole('button', { name: 'Pseudo-locale' }).click();
  const targets = await allTargets(page);
  for (const target of targets) {
    await page.goto(`/#gallery/${target}`);
    const frame = page.getByTestId('gallery-frame');
    await expect(frame).toBeVisible();
    // A missing key would render as its dotted key path, unaccented.
    const text = await frame.innerText();
    expect.soft(text, target).not.toMatch(/\b[a-z]+\.[a-z]+\.[a-zA-Z.]+\b/);
  }
});
