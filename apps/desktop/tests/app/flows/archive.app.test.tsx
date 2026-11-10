import { screen } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderApp } from '../../support/app';

async function chooseFile(user: UserEvent, which: 'Save to a file' | 'Open a file'): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'What are we doing on this computer?' });
  await user.click(screen.getByRole('button', { name: 'Use an encrypted file instead' }));
  await user.click(screen.getByRole('button', { name: new RegExp(which) }));
}

describe('encrypted file', () => {
  it('saves the selection with a confirmed, strong passphrase to the chosen path', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const exportArchive = vi.spyOn(port, 'exportArchive');
    await chooseFile(user, 'Save to a file');
    await user.click(await screen.findByRole('button', { name: 'Continue' }));

    const save = await screen.findByRole('button', { name: 'Save file' });
    await user.type(screen.getByLabelText('Passphrase'), 'orbit lemon tiger canvas');
    expect(save).toBeDisabled();
    await user.type(screen.getByLabelText('Type it again'), 'orbit lemon tiger canvas');
    const where = screen.getByLabelText('Save to');
    await user.clear(where);
    await user.type(where, '/tmp/mine.carrova');
    expect(screen.getByText(/about a second/)).toBeInTheDocument();
    await user.selectOptions(screen.getByRole('combobox', { name: 'Protection' }), 'strong');
    expect(screen.getByText(/2–3 times as long/)).toBeInTheDocument();
    await user.click(save);

    expect(exportArchive).toHaveBeenCalledWith(
      expect.any(String),
      '/tmp/mine.carrova',
      'orbit lemon tiger canvas',
      'strong',
    );
    expect(await screen.findByText('Your setup is in one file.')).toBeInTheDocument();
  });

  it('offers the native dialog only when the host has one', async () => {
    const user = userEvent.setup();
    const pickSavePath = vi.fn(() => Promise.resolve('/media/usb/setup.carrova'));
    await renderApp({ returning: true, host: { pickSavePath } });
    await chooseFile(user, 'Save to a file');
    await user.click(await screen.findByRole('button', { name: 'Continue' }));
    await user.click(await screen.findByRole('button', { name: 'Choose where…' }));
    expect(screen.getByLabelText('Save to')).toHaveValue('/media/usb/setup.carrova');
  });

  it('says so when the passphrase does not open the file, then opens it into the review', async () => {
    const user = userEvent.setup();
    await renderApp({ returning: true });
    await chooseFile(user, 'Open a file');
    await user.type(await screen.findByLabelText('File'), '/tmp/mine.carrova');
    await user.type(screen.getByLabelText('Passphrase'), 'wrong');
    await user.click(screen.getByRole('button', { name: 'Open' }));
    expect(
      await screen.findByText(
        "That passphrase doesn't open this file. Check caps lock and try again.",
      ),
    ).toBeInTheDocument();

    await user.clear(screen.getByLabelText('Passphrase'));
    await user.type(screen.getByLabelText('Passphrase'), 'right one');
    await user.click(screen.getByRole('button', { name: 'Open' }));
    expect(await screen.findByRole('button', { name: 'Approve and move' })).toBeInTheDocument();
  });
});
