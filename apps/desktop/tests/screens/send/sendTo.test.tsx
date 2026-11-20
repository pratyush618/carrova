import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { sendToChoose, sendToEmpty } from '@/screens/send/fixtures';
import { SendToSheet } from '@/screens/send/SendToSheet';

const intents = () => ({
  onSend: vi.fn(),
  onWake: vi.fn(),
  onUseCode: vi.fn(),
  onCancel: vi.fn(),
});

describe('Send your setup to…', () => {
  it('lists trusted computers to send to without a code, away ones greyed', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<SendToSheet model={sendToChoose} on={on} />);
    expect(screen.getByText('12 apps · 340 MB · includes keys')).toBeInTheDocument();
    const trusted = screen.getByRole('list', { name: 'No code needed' });
    const names = within(trusted)
      .getAllByRole('button')
      .map((b) => b.getAttribute('aria-label'));
    expect(names).toEqual([
      'Send to Desk PC',
      "Send to Ana's laptop",
      'Wake Old ThinkPad',
      'Use a code',
    ]);
    await user.click(screen.getByRole('button', { name: 'Send to Desk PC' }));
    expect(on.onSend).toHaveBeenCalledWith(expect.objectContaining({ id: 'd2' }));
    await user.click(screen.getByRole('button', { name: 'Wake Old ThinkPad' }));
    expect(on.onWake).toHaveBeenCalledWith(expect.objectContaining({ id: 'o3' }));
  });

  it('asks a new computer for a code once', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<SendToSheet model={sendToChoose} on={on} />);
    const fresh = screen.getByRole('list', { name: 'Needs a code once' });
    expect(within(fresh).getByText('new-mbp')).toBeInTheDocument();
    expect(within(fresh).getByText(/After this move, no code/)).toBeInTheDocument();
    await user.click(within(fresh).getByRole('button', { name: 'Use a code' }));
    expect(on.onUseCode).toHaveBeenCalledOnce();
  });

  it('offers a code when no computer is trusted yet', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<SendToSheet model={sendToEmpty} on={on} />);
    expect(screen.getByText('No computers you trust yet')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Send with a code' }));
    expect(on.onUseCode).toHaveBeenCalledOnce();
  });
});
