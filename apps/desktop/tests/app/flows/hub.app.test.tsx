import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { importSession } from '@/ipc/fake/fixtures';
import { renderApp } from '../../support/app';

async function openHub(user: UserEvent, page: 'History' | 'Apps' | 'Settings'): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' });
  await user.click(screen.getByRole('button', { name: page }));
}

describe('history', () => {
  it('lists past moves and undoes one after the preview', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const rollback = vi.spyOn(port, 'rollback');
    await openHub(user, 'History');
    await user.click(
      await screen.findByRole('button', { name: /Opened a file from laptop\.carrova/ }),
    );
    const sheet = screen.getByRole('dialog', { name: 'Opened a file from laptop.carrova' });
    await user.click(within(sheet).getByRole('button', { name: 'Undo this move' }));
    // The engine's preview of what undo would do.
    expect(await screen.findByText('12 files removed')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Undo the move' }));

    expect(rollback).toHaveBeenCalledWith(importSession);
    expect(
      await screen.findByText('Undone. This computer is back to how it was.'),
    ).toBeInTheDocument();
  });

  it('forgets a move only after the user confirms', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const dismiss = vi.spyOn(port, 'dismiss');
    await openHub(user, 'History');
    await user.click(await screen.findByRole('button', { name: /Opened a file/ }));
    await user.click(screen.getByRole('button', { name: 'Remove' }));
    const dialog = screen.getByRole('alertdialog', { name: 'Forget this move?' });
    expect(within(dialog).getByRole('button', { name: 'Keep it' })).toHaveFocus();
    await user.click(within(dialog).getByRole('button', { name: 'Forget it' }));

    expect(dismiss).toHaveBeenCalledWith(importSession);
  });
});

describe('apps and settings', () => {
  it('opens a recipe with its tier and items', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await openHub(user, 'Apps');
    const grid = await screen.findByRole('list', { name: 'Apps' });
    const [first] = within(grid).getAllByRole('button');
    if (first === undefined) throw new Error('empty catalogue');
    await user.click(first);
    const sheet = screen.getByRole('dialog');
    expect(within(sheet).getByText('Trust')).toBeInTheDocument();
    expect(within(sheet).getByRole('list', { name: 'What moves' })).toBeInTheDocument();
  });

  it('saves a theme choice and checks recipes on request', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const save = vi.spyOn(port, 'saveSettings');
    await openHub(user, 'Settings');
    await user.selectOptions(await screen.findByRole('combobox', { name: 'Appearance' }), 'dark');
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ theme: 'dark' }));
    expect(document.documentElement.dataset['theme']).toBe('dark');

    await user.click(screen.getByRole('button', { name: 'Check now' }));
    expect(await screen.findByText('You have the newest list.')).toBeInTheDocument();
  });

  it('checks for a Carrova update only when asked', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const check = vi.spyOn(port, 'checkUpdate');
    await openHub(user, 'Settings');
    expect(check).not.toHaveBeenCalled();
    await user.click(await screen.findByRole('button', { name: /^Updates/ }));
    expect(check).toHaveBeenCalledOnce();
    expect(
      await screen.findByRole('dialog', { name: 'Carrova is up to date' }),
    ).toBeInTheDocument();
  });
});
