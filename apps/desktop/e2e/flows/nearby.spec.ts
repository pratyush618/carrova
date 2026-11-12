import { expect, test } from '@playwright/test';
import { consoleErrors, expectAccessible, openApp } from './support';

// The fake `nearby` network: Ana's laptop and Desk PC (known) and new-mbp
// answer; Old ThinkPad (with a wake hint) and Work laptop are away.
test('nearby shows the network, wakes and forgets away devices, and shares visibility with Settings', async ({
  page,
}) => {
  const errors = consoleErrors(page);
  await openApp(page, 'nearby');
  const sidebar = page.getByRole('navigation', { name: 'Main' });
  await sidebar.getByRole('button', { name: 'Nearby' }).click();

  const map = page.getByRole('list', { name: 'Nearby devices' });
  await expect(map.getByRole('button', { name: /Ana's laptop/ })).toBeVisible();
  await expect(map.getByRole('button', { name: /new-mbp/ })).toBeVisible();
  await expect(map.getByRole('button', { name: /Old ThinkPad/ })).toBeVisible();
  await expectAccessible(page, 'map');

  await map.getByRole('button', { name: /Ana's laptop/ }).click();
  const card = page.getByRole('region', { name: "Ana's laptop" });
  await expect(card.getByText('7F3A·19C2')).toBeVisible();
  await expectAccessible(page, 'device card');

  await page.getByRole('radio', { name: 'List' }).click();
  await page.getByRole('button', { name: 'Wake: Old ThinkPad' }).click();
  await expect(page.getByText(/Wake sent to Old ThinkPad/)).toBeVisible();
  await expect(page.getByRole('button', { name: 'Wake: Work laptop' })).toHaveCount(0);
  await page.getByRole('button', { name: 'Forget: Work laptop' }).click();
  await expect(page.getByRole('button', { name: 'Forget: Work laptop' })).toHaveCount(0);
  await expectAccessible(page, 'list');

  await page.getByRole('button', { name: /Visible to known devices/ }).click();
  await expectAccessible(page, 'visibility menu');
  await page.getByRole('button', { name: /^No one/ }).click();
  await expect(page.getByRole('button', { name: /Visible to no one/ })).toBeVisible();

  await page.getByRole('button', { name: /Visible to no one/ }).click();
  await page.getByRole('button', { name: /^Nearby settings/ }).click();
  const visibility = page.getByRole('radiogroup', { name: 'Visibility' });
  await expect(visibility.getByRole('radio', { name: 'No one' })).toHaveAttribute(
    'aria-checked',
    'true',
  );
  await expect(page.getByText('3 remembered · rename or forget')).toBeVisible();
  await expectAccessible(page, 'nearby settings');

  await page.getByRole('button', { name: /Known devices/ }).click();
  const sheet = page.getByRole('dialog', { name: 'Known devices' });
  await sheet.getByRole('button', { name: 'Forget all' }).click();
  const confirm = page.getByRole('alertdialog', { name: 'Forget every known device?' });
  await confirm.getByRole('button', { name: 'Forget all' }).click();
  await expect(sheet.getByText('No devices remembered yet.')).toBeVisible();
  expect(errors).toEqual([]);
});

test('settings search finds the Wake-on-LAN row', async ({ page }) => {
  await openApp(page, 'nearby');
  await page.getByRole('button', { name: 'Settings', exact: true }).click();
  await page.getByRole('searchbox', { name: 'Search settings' }).fill('wake');
  await page.getByRole('button', { name: /Wake-on-LAN/ }).click();
  await expect(page.getByRole('switch', { name: /Wake-on-LAN/ })).toBeVisible();
});

test('every device on the map stays inside the field at the smallest window', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 560 });
  await openApp(page, 'nearby');
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Nearby' })
    .click();
  const field = await page.locator('.s-near__field').boundingBox();
  const devices = page.getByRole('list', { name: 'Nearby devices' }).getByRole('button');
  await expect(devices).toHaveCount(5);
  for (const device of await devices.all()) {
    const box = await device.boundingBox();
    expect(box && field).toBeTruthy();
    if (box === null || field === null) continue;
    expect.soft(box.x).toBeGreaterThanOrEqual(field.x);
    expect.soft(box.y).toBeGreaterThanOrEqual(field.y);
    expect.soft(box.x + box.width).toBeLessThanOrEqual(field.x + field.width);
    expect.soft(box.y + box.height).toBeLessThanOrEqual(field.y + field.height);
  }
});

interface Box {
  x: number;
  y: number;
  width: number;
  height: number;
}
const apart = (a: Box, b: Box) =>
  a.x + a.width <= b.x || b.x + b.width <= a.x || a.y + a.height <= b.y || b.y + b.height <= a.y;

for (const size of [
  { width: 1100, height: 520 },
  { width: 720, height: 560 },
  { width: 1440, height: 900 },
]) {
  test(`no device sits under another, the centre, the pill or the card at ${String(size.width)}x${String(size.height)}`, async ({
    page,
  }) => {
    await page.setViewportSize(size);
    await openApp(page, 'nearby');
    await page
      .getByRole('navigation', { name: 'Main' })
      .getByRole('button', { name: 'Nearby' })
      .click();
    const devices = page.getByRole('list', { name: 'Nearby devices' }).getByRole('button');
    await expect(devices).toHaveCount(5);
    await devices.first().click();
    const card = page.getByRole('region', { name: "Ana's laptop" });
    await expect(card).toBeVisible();
    const others = new Map<string, Box | null>([
      ['pill', await page.getByRole('button', { name: /^Visible to/ }).boundingBox()],
      ['card', await card.boundingBox()],
      ['centre', await page.locator('.s-near__me').boundingBox()],
    ]);
    const boxes: [string, Box | null][] = [];
    for (const device of await devices.all()) {
      boxes.push([(await device.textContent()) ?? '', await device.boundingBox()]);
    }
    for (const [i, [name, box]] of boxes.entries()) {
      expect(box).not.toBeNull();
      if (box === null) continue;
      const rivals = [...others, ...boxes.slice(i + 1)];
      for (const [rival, over] of rivals) {
        expect(over).not.toBeNull();
        if (over !== null) expect.soft(apart(box, over), `${name} overlaps ${rival}`).toBe(true);
      }
    }
    await page.screenshot({
      path: `/tmp/carrova-nearby-${String(size.width)}x${String(size.height)}.png`,
    });
  });
}
