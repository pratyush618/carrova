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

const sizes = [
  { width: 720, height: 560 },
  { width: 1100, height: 520 },
  { width: 1440, height: 900 },
];

// `nearby` fits whole (2 inner + 3 outer); `nearby-crowded` (7 inner + 14 outer) fills every slot the field
// opens (spots.ts: 4+3, 5+8, 6+12) and sums up the rest in the chip.
const cases = [
  ...sizes.map((size) => ({ scenario: 'nearby', size, shown: 5 })),
  ...sizes.map((size, i) => ({ scenario: 'nearby-crowded', size, shown: [7, 13, 18][i] ?? 0 })),
];

for (const { scenario, size, shown } of cases)
  test(`no device sits under another, the centre, the pill or the card at ${String(size.width)}x${String(size.height)} (${scenario})`, async ({
    page,
  }) => {
    await page.setViewportSize(size);
    await openApp(page, scenario);
    await page
      .getByRole('navigation', { name: 'Main' })
      .getByRole('button', { name: 'Nearby' })
      .click();
    const map = page.getByRole('list', { name: 'Nearby devices' });
    const devices = map.locator('button[data-key]');
    await expect(devices).toHaveCount(shown);
    const total = scenario === 'nearby' ? 5 : 21;
    const chip = map.getByRole('button', { name: /more: show all/ });
    if (shown < total) await expect(chip).toHaveText(`+${String(total - shown)} more`);
    else await expect(chip).toHaveCount(0);
    await devices.first().click();
    const card = page.getByRole('region', { name: "Ana's laptop" });
    await expect(card).toBeVisible();
    const others = new Map<string, Box | null>([
      ['pill', await page.getByRole('button', { name: /^Visible to/ }).boundingBox()],
      ['card', await card.boundingBox()],
      ['centre', await page.locator('.s-near__me').boundingBox()],
    ]);
    const boxes: [string, Box | null][] = [];
    for (const button of await map.getByRole('button').all()) {
      boxes.push([(await button.textContent()) ?? '', await button.boundingBox()]);
    }
    // The page, not the field: on a short window the field runs on below what shows.
    const field = await page.locator('.s-near').boundingBox();
    expect(field).not.toBeNull();
    for (const [i, [name, box]] of boxes.entries()) {
      expect(box).not.toBeNull();
      if (box === null || field === null) continue;
      expect
        .soft(
          box.x >= field.x &&
            box.x + box.width <= field.x + field.width &&
            box.y >= field.y &&
            box.y + box.height <= field.y + field.height,
          `${name} leaves the page`,
        )
        .toBe(true);
      const rivals = [...others, ...boxes.slice(i + 1)];
      for (const [rival, over] of rivals) {
        expect(over).not.toBeNull();
        if (over !== null) expect.soft(apart(box, over), `${name} overlaps ${rival}`).toBe(true);
      }
    }
    // Rings keep their meaning: devices answering now sit nearer the centre than the rest.
    const centre = others.get('centre');
    expect(centre).toBeTruthy();
    if (centre) {
      const away = async (ring: 'inner' | 'outer') => {
        const out: number[] = [];
        for (const button of await map.locator(`.s-near__spot--${ring} button`).all()) {
          const box = await button.boundingBox();
          if (box)
            out.push(
              Math.hypot(
                box.x + box.width / 2 - (centre.x + centre.width / 2),
                box.y + box.height / 2 - (centre.y + centre.height / 2),
              ),
            );
        }
        return out;
      };
      expect(Math.max(...(await away('inner')))).toBeLessThan(Math.min(...(await away('outer'))));
    }
  });

test('the chip opens the list with every device, the long name whole', async ({ page }) => {
  await page.setViewportSize({ width: 720, height: 560 });
  await openApp(page, 'nearby-crowded');
  await page
    .getByRole('navigation', { name: 'Main' })
    .getByRole('button', { name: 'Nearby' })
    .click();
  const long = 'The shared render workstation in the back office';
  const pill = page.getByRole('button', { name: new RegExp(`^${long}`) });
  await expect(pill).toHaveAttribute('title', long);
  expect((await pill.boundingBox())?.width).toBeLessThanOrEqual(164);
  await expectAccessible(page, 'crowded map');
  await page.getByRole('button', { name: '+14 more: show all 21 devices in the list' }).click();
  await expect(page.getByRole('radio', { name: 'List' })).toBeChecked();
  const here = page.getByRole('list', { name: 'Here now' });
  await expect(here.getByRole('listitem')).toHaveCount(21);
  await expect(here.getByText(long)).toBeVisible();
});
