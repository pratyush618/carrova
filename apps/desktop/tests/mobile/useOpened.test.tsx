import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone } from '../support/phone';

describe('links the phone app is opened with', () => {
  it('opened send-clipboard link opens the send sheet', async () => {
    await renderPhone({ scenario: 'linked', opened: ['carrova://send-clipboard'] });
    expect(await screen.findByRole('dialog', { name: 'Send clipboard' })).toBeInTheDocument();
  });

  it('opened shared file opens the send pick with its path', async () => {
    const user = userEvent.setup();
    const shared = '/data/user/0/org.byteveda.Carrova/files/cache/shared/1/IMG 2041.jpg';
    const { port } = await renderPhone({
      scenario: 'linked',
      opened: [`file://${shared.replace(' ', '%20')}`],
    });
    const send = vi.spyOn(port, 'sendFiles');
    const sheet = await screen.findByRole('dialog', { name: 'Send 1 file' });
    expect(within(sheet).getByText('IMG 2041.jpg')).toBeInTheDocument();

    await user.click(within(sheet).getByRole('button', { name: 'Send' }));
    expect(send).toHaveBeenCalledWith('d2', [shared]);
  });

  it('opened web link offers to open it on a computer', async () => {
    const user = userEvent.setup();
    const url = 'https://carrova.org/docs/pairing';
    const { port } = await renderPhone({ scenario: 'linked', opened: [url] });
    const handoff = vi.spyOn(port, 'handoffUrl');
    const sheet = await screen.findByRole('dialog', { name: 'Open on a computer' });

    await user.click(within(sheet).getByRole('button', { name: 'Send' }));
    expect(handoff).toHaveBeenCalledWith('d2', url);
    expect(await screen.findByText('Sent to Desk PC')).toBeInTheDocument();
  });
});
