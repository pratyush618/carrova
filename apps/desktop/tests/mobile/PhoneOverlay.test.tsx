import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../support/app';

describe('text shared to the phone', () => {
  it('opens a send sheet with the text, and sends it to the computer picked', async () => {
    const user = userEvent.setup();
    const port = createFakePort({
      stepMs: 0,
      scenario: 'linked',
      os: 'android',
      opened: ['data:text/plain,Meet%20at%207%20%E2%9C%93'],
    });
    const send = vi.spyOn(port, 'sendText');
    await renderApp({ port, returning: true });
    const sheet = await screen.findByRole('dialog', { name: 'Send text' });
    expect(within(sheet).getByText('Meet at 7 ✓')).toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Send' }));
    expect(send).toHaveBeenCalledWith('d2', 'Meet at 7 ✓');
    expect(await screen.findByText('Clipboard sent to Desk PC')).toBeInTheDocument();
  });
});
