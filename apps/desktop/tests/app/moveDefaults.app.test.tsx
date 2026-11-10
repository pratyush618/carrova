import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderApp } from '../support/app';

async function openSettings(user: UserEvent): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' });
  await user.click(screen.getByRole('button', { name: 'Settings' }));
}

describe('suggested defaults for new moves', () => {
  it('pre-fill the review of the next move, which can still change them', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const plan = vi.spyOn(port, 'plan');
    await openSettings(user);
    await user.selectOptions(
      await screen.findByRole('combobox', { name: 'When a file already exists' }),
      'keep-both',
    );
    await user.selectOptions(
      screen.getByRole('combobox', { name: 'Keep the undo point' }),
      'discard_on_success',
    );
    await user.click(screen.getByRole('button', { name: 'Home' }));

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
    await openSettings(user);
    const group = (await screen.findByRole('heading', { name: 'New moves start with' }))
      .parentElement;
    if (group === null) throw new Error('no group');
    await user.click(within(group).getByRole('switch', { name: 'Bring caches' }));

    expect(save.mock.calls.at(-1)?.[0].move_defaults.include_caches).toBe(true);
  });
});
