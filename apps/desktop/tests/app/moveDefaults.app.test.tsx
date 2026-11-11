import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderApp } from '../support/app';

async function openSettings(user: UserEvent, section: string): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
  await user.click(screen.getByRole('button', { name: 'Settings' }));
  await user.click(
    within(screen.getByRole('navigation', { name: 'Settings sections' })).getByRole('button', {
      name: section,
    }),
  );
}

describe('suggested defaults for new moves', () => {
  it('pre-fill the review of the next move, which can still change them', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const plan = vi.spyOn(port, 'plan');
    await openSettings(user, 'Moves');
    const rule = await screen.findByRole('radiogroup', { name: 'When a file already exists' });
    await user.click(within(rule).getByRole('radio', { name: 'Keep both' }));
    const retention = screen.getByRole('radiogroup', { name: 'Keep the undo point' });
    await user.click(within(retention).getByRole('radio', { name: 'Until the move succeeds' }));
    // Back from Settings lands on Home.
    await user.click(screen.getByRole('button', { name: 'Back from settings' }));

    await user.click(await screen.findByRole('button', { name: /Receive/ }));
    await user.type(await screen.findByRole('textbox', { name: 'Pairing code' }), '7-acid-lemon');
    await user.click(screen.getByRole('button', { name: 'Connect' }));
    await screen.findByRole('button', { name: 'Approve and move' });

    expect(plan.mock.calls[0]?.[1]).toMatchObject({
      default_rule: 'keep-both',
      retention: 'discard_on_success',
    });
  });

  it('are saved with the other settings', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const save = vi.spyOn(port, 'saveSettings');
    await openSettings(user, 'Sending');
    await user.click(await screen.findByRole('switch', { name: 'Bring caches' }));

    expect(save.mock.calls.at(-1)?.[0].move_defaults.include_caches).toBe(true);
  });
});
