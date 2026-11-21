/**
 * The call window: a call on a linked phone opens it, and it shows the call.
 * No phone needed: the engine's `call_view` event is played in from the main
 * window, as the link would send it. A call ringing here also plays the
 * ringtone, which loads the webview's media stack.
 */
import { $, browser, expect } from '@wdio/globals';

// RFC 8032 test 1's public key: a valid device id.
const PHONE = 'd75a980182b10ab7d54bfed3c964073a0ee172f3daa62325af021a68f707511a';

/** Play `view` in as the link's `call_view` event. */
async function playCallView(view: Record<string, unknown>): Promise<void> {
  await browser.execute(
    (payload) =>
      (
        window as unknown as {
          __TAURI_INTERNALS__: { invoke: (cmd: string, args: unknown) => Promise<unknown> };
        }
      ).__TAURI_INTERNALS__.invoke('plugin:event|emit', { event: 'carrova://event', payload }),
    { kind: 'engine', session: 'link', event: { event: 'call_view', ...view } },
  );
}

describe('Call window (real app)', () => {
  it('opens on a call ringing here and shows the caller', async () => {
    await $('h1').waitForDisplayed();
    const main = await browser.getWindowHandle();
    await playCallView({
      kind: 'ringing',
      device: PHONE,
      number: '+15550101',
      name: 'Sam',
      ring_here: true,
    });
    await browser.waitUntil(async () => (await browser.getWindowHandles()).length === 2, {
      timeoutMsg: 'no call window',
    });
    const call = (await browser.getWindowHandles()).find((h) => h !== main) ?? '';
    await browser.switchToWindow(call);
    await expect($('h1')).toHaveText('Sam');
    await expect($('button=Answer')).toBeDisplayed();
    // Still answering once the ringtone has had time to start.
    await browser.pause(2000);
    await expect($('h1')).toHaveText('Sam');
    await browser.switchToWindow(main);
  });
});
