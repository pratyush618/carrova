/**
 * The real app, end to end through its bridge: window, first screens, the
 * engine answering over Tauri IPC, and no script errors on the way.
 *
 * Strings are the English catalogue (`apps/desktop/src/i18n/en`).
 */
import { $, $$, browser, expect } from '@wdio/globals';
import { existsSync } from 'node:fs';
import { join } from 'node:path';

/** At least the recipes in the repo's `recipes/` (debug builds read the checkout). */
const MIN_RECIPES = 15;

/** Errors seen by the page since `watchErrors` ran (script errors, console.error, CSP). */
async function pageErrors(): Promise<string[]> {
  return browser.execute(() => (window as unknown as { e2eErrors?: string[] }).e2eErrors ?? []);
}

/**
 * Records uncaught errors, rejections, `console.error` and CSP violations in
 * `window.e2eErrors`. WebKitWebDriver has no log endpoint, so this is how
 * console errors reach the test; the first paint happens before it is
 * installed and is covered by the screens rendering at all.
 */
async function watchErrors(): Promise<void> {
  await browser.execute(() => {
    const errors: string[] = [];
    (window as unknown as { e2eErrors: string[] }).e2eErrors = errors;
    window.addEventListener('error', (e) => errors.push(`error: ${e.message}`));
    window.addEventListener('unhandledrejection', (e) =>
      errors.push(`unhandled rejection: ${String(e.reason)}`),
    );
    document.addEventListener('securitypolicyviolation', (e) =>
      errors.push(`CSP: ${e.violatedDirective} ${e.blockedURI}`),
    );
    const original = console.error.bind(console);
    console.error = (...args: unknown[]) => {
      errors.push(`console.error: ${args.map(String).join(' ')}`);
      original(...args);
    };
  });
}

/** Opens a hub page from the top bar's main navigation. */
async function openFromNav(label: string): Promise<void> {
  await $('nav[aria-label="Main"]').$(`button*=${label}`).click();
}

describe('Carrova desktop (real app)', () => {
  before(async () => {
    await $('h1').waitForDisplayed();
    await watchErrors();
  });

  it('opens its window with the Carrova title', async () => {
    expect(await browser.getWindowHandles()).toHaveLength(1);
    await expect(browser).toHaveTitle('Carrova');
  });

  it('runs inside Tauri, not over the in-memory fake port', async () => {
    const inTauri = await browser.execute(() => '__TAURI_INTERNALS__' in window);
    expect(inTauri).toBe(true);
  });

  it('starts on Home and asks once about error reports', async () => {
    await expect($('h1')).toHaveText('Bring your setup with you.');
    const question = $('h2=Send error reports if Carrova crashes?');
    await expect(question).toBeDisplayed();
    await $('button=No thanks').click();
    await expect(question).not.toBeDisplayed();
  });

  it('lists the recipes the engine loaded in the Apps catalogue', async () => {
    await openFromNav('Apps');
    const cards = $$('ul[aria-label="Apps"] > li');
    await browser.waitUntil(async () => (await cards.length) >= MIN_RECIPES, {
      timeoutMsg: `expected at least ${String(MIN_RECIPES)} recipes in the catalogue`,
    });
  });

  it('shows which recipes are in use in Settings', async () => {
    await openFromNav('Settings');
    await expect($('h1')).toHaveText('Settings');
    // Debug build, no installed bundle: the checkout's unsigned tree.
    // Nothing in an unsigned tree counts as verified.
    await expect($('main')).toHaveText(/Development folder: [1-9]\d* unverified, not signed/);
  });

  it('keeps its state in the isolated home', () => {
    const sandbox = process.env['CARROVA_E2E_SANDBOX'];
    expect(sandbox).toBeDefined();
    expect(existsSync(join(sandbox ?? '', 'state', 'carrova'))).toBe(true);
  });

  it('logged no errors', async () => {
    expect(await pageErrors()).toEqual([]);
  });
});
