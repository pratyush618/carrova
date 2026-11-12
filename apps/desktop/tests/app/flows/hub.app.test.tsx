import { screen, waitFor, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { importSession } from '@/ipc/fake/fixtures';
import { renderApp } from '../../support/app';

async function openSection(user: UserEvent, name: string): Promise<void> {
  await user.click(
    within(await screen.findByRole('navigation', { name: 'Settings sections' })).getByRole(
      'button',
      { name },
    ),
  );
}

async function openHub(user: UserEvent, page: 'History' | 'Apps' | 'Settings'): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
  await user.click(screen.getByRole('button', { name: page }));
}

describe('history', () => {
  it('opens the newest move beside the list and undoes it after the preview', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const rollback = vi.spyOn(port, 'rollback');
    await openHub(user, 'History');
    expect(
      await screen.findByRole('button', { name: /Opened a file from laptop\.carrova/ }),
    ).toHaveAttribute('aria-current', 'true');
    const detail = screen.getByRole('complementary', { name: 'This move' });
    // The engine's preview of what undo would do, in the rail and again in the dialog.
    expect(await within(detail).findByText('12 new files')).toBeInTheDocument();
    await user.click(within(detail).getByRole('button', { name: 'Undo this move…' }));
    expect(
      await within(await screen.findByRole('dialog')).findByText('12 new files'),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Undo move' }));

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
    await user.click(await screen.findByRole('button', { name: 'Forget this move…' }));
    const dialog = screen.getByRole('alertdialog', { name: 'Forget this move?' });
    expect(within(dialog).getByRole('button', { name: 'Keep it' })).toHaveFocus();
    await user.click(within(dialog).getByRole('button', { name: 'Forget it' }));

    expect(dismiss).toHaveBeenCalledWith(importSession);
    expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument();
  });

  it('undoes a recent move straight from Home', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await user.click(
      await screen.findByRole('button', { name: 'Undo: Opened a file from laptop.carrova' }),
    );
    expect(await screen.findByRole('dialog', { name: 'Undo this move?' })).toBeInTheDocument();
  });
});

describe('apps and settings', () => {
  it('opens a recipe in the detail pane with its tier and items', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await openHub(user, 'Apps');
    const list = await screen.findByRole('list', { name: 'Apps' });
    const [first] = within(list).getAllByRole('button', { pressed: false });
    if (first === undefined) throw new Error('empty catalogue');
    await user.click(first);
    const detail = screen.getByRole('complementary', { name: 'App details' });
    expect(within(detail).getByText(/signed\.$|not signed\.$/)).toBeInTheDocument();
    expect(within(detail).getByRole('list', { name: 'What moves' })).toBeInTheDocument();
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('saves a theme choice and checks recipes on request', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const save = vi.spyOn(port, 'saveSettings');
    await openHub(user, 'Settings');
    const theme = await screen.findByRole('radiogroup', { name: 'Theme' });
    await user.click(within(theme).getByRole('radio', { name: 'Dark' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ theme: 'dark' }));
    expect(document.documentElement.dataset['theme']).toBe('dark');

    await openSection(user, 'Updates');
    await user.click(screen.getByRole('button', { name: 'Check now' }));
    expect(await screen.findByText('You have the newest list.')).toBeInTheDocument();
  });

  it('starts at login through the background command, not a plain save', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const background = vi.spyOn(port, 'setBackground');
    const save = vi.spyOn(port, 'saveSettings');
    await openHub(user, 'Settings');
    await user.click(await screen.findByRole('switch', { name: 'Start at login' }));
    expect(background).toHaveBeenCalledWith({ start_at_login: true });
    expect(save).not.toHaveBeenCalled();
    expect(screen.getByRole('switch', { name: 'Start at login' })).toBeChecked();
  });

  it('offers a newer recipe list after a check and installs it on request', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, scenario: 'recipes-update' });
    const install = vi.spyOn(port, 'recipesUpdate');
    await openHub(user, 'Settings');
    await openSection(user, 'Updates');
    await user.click(await screen.findByRole('button', { name: 'Check now' }));
    expect(await screen.findByText('List 2 is ready.')).toBeInTheDocument();
    expect(install).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Install update' }));
    expect(await screen.findByText('Updated to list 2.')).toBeInTheDocument();
    expect(install).toHaveBeenCalledOnce();
  });

  it('checks for a Carrova update only when asked', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const check = vi.spyOn(port, 'checkUpdate');
    await openHub(user, 'Settings');
    await openSection(user, 'Updates');
    expect(check).not.toHaveBeenCalled();
    await user.click(within(screen.getByRole('main')).getByRole('button', { name: /^Updates/ }));
    expect(check).toHaveBeenCalledOnce();
    expect(
      await screen.findByRole('dialog', { name: 'Carrova is up to date' }),
    ).toBeInTheDocument();
  });
});

describe('sidebar', () => {
  it('says at its foot that development recipes are not signed', async () => {
    await renderApp({ returning: true });
    await waitFor(() => {
      expect(screen.getByRole('navigation', { name: 'Main' })).toHaveTextContent(
        'Recipes not signedDevelopment folder · 21 apps',
      );
    });
  });
});

describe('the one-time file-manager question', () => {
  const question = 'Add “Send with Carrova” to your file manager?';

  it('answer no never asks again', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, settings: { file_manager_asked: false } });
    const save = vi.spyOn(port, 'saveSettings');
    const entries = vi.spyOn(port, 'setFileManagerEntries');
    await screen.findByRole('region', { name: question });
    await user.click(screen.getByRole('button', { name: 'Not now' }));

    expect(save).toHaveBeenCalledWith(expect.objectContaining({ file_manager_asked: true }));
    expect(entries).not.toHaveBeenCalled();
    expect(screen.queryByRole('region', { name: question })).not.toBeInTheDocument();
    await openHub(user, 'Settings');
    await user.click(screen.getByRole('button', { name: 'Back from settings' }));
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    expect(screen.queryByRole('region', { name: question })).not.toBeInTheDocument();
  });

  it('writes the entries on yes, and Settings shows them on', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true, settings: { file_manager_asked: false } });
    const entries = vi.spyOn(port, 'setFileManagerEntries');
    await screen.findByRole('region', { name: question });
    await user.click(screen.getByRole('button', { name: 'Add it' }));

    expect(entries).toHaveBeenCalledWith(true);
    expect(screen.queryByRole('region', { name: question })).not.toBeInTheDocument();
    await openHub(user, 'Settings');
    await openSection(user, 'Sending');
    expect(
      await screen.findByRole('switch', { name: 'Show “Send with Carrova” in the file manager' }),
    ).toHaveAttribute('aria-checked', 'true');
  });

  it('hidden while reports question open', async () => {
    await renderApp({ settings: { file_manager_asked: false } });
    await screen.findByRole('heading', { name: 'Send error reports if Carrova crashes?' });
    expect(screen.queryByRole('region', { name: question })).not.toBeInTheDocument();
  });
});
