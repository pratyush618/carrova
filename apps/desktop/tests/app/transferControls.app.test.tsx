import { screen } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { CarrovaPort } from '@/engine/types';
import { renderApp } from '../support/app';

/** Receive over the LAN, bring every sensitive item, approve: the transfer runs. */
async function startTransfer(user: UserEvent): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
  await user.click(screen.getByRole('button', { name: /Receive/ }));
  await user.type(await screen.findByRole('textbox', { name: 'Pairing code' }), '7-acid-lemon');
  await user.click(screen.getByRole('button', { name: 'Connect' }));
  const approve = await screen.findByRole(
    'button',
    { name: 'Approve and move' },
    { timeout: 5000 },
  );
  for (;;) {
    const [open] = screen.queryAllByRole('radio', { name: 'Bring them', checked: false });
    if (open === undefined) break;
    await user.click(open);
  }
  await vi.waitFor(() => {
    expect(approve).toBeEnabled();
  });
  await user.click(approve);
}

/**
 * The fake's clock: instant until the transfer runs (its first progress or
 * pause), then held, so the controls stay on screen however slowly the
 * test machine gets to them. The fake's control commands do not wait on it.
 */
function holdOnceTransferring(): {
  sleep: () => Promise<void>;
  watch: (port: CarrovaPort) => Promise<void>;
} {
  let held = false;
  return {
    sleep: () => (held ? new Promise<void>(() => undefined) : Promise.resolve()),
    watch: async (port) => {
      await port.subscribe((e) => {
        if (e.kind === 'engine' && ['progress', 'transfer_paused'].includes(e.event.event)) {
          held = true;
        }
      });
    },
  };
}

const spy = (port: CarrovaPort) => ({
  paused: vi.spyOn(port, 'setTransferPaused'),
  ignored: vi.spyOn(port, 'ignoreFreeSpace'),
});

describe('controls of a running LAN transfer', () => {
  it('continues a transfer paused for space only after the warning', async () => {
    const user = userEvent.setup();
    const clock = holdOnceTransferring();
    const { port } = await renderApp({
      returning: true,
      scenario: 'disk-full',
      sleep: clock.sleep,
    });
    await clock.watch(port);
    const calls = spy(port);
    await startTransfer(user);
    await user.click(
      await screen.findByRole('button', { name: 'Ignore the space check' }, { timeout: 5000 }),
    );
    expect(calls.ignored).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Ignore and continue' }));

    expect(calls.ignored).toHaveBeenCalledOnce();
  });

  it('pauses and resumes at the user’s request', async () => {
    const user = userEvent.setup();
    const clock = holdOnceTransferring();
    const { port } = await renderApp({ returning: true, sleep: clock.sleep });
    await clock.watch(port);
    const calls = spy(port);
    await startTransfer(user);
    await user.click(await screen.findByRole('button', { name: 'Pause' }, { timeout: 5000 }));
    await user.click(await screen.findByRole('button', { name: 'Resume' }));

    expect(calls.paused.mock.calls.map((c) => c[1])).toEqual([true, false]);
  });
});
