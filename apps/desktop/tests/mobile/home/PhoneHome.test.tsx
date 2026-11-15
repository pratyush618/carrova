import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone } from '../../support/phone';

describe('the phone Home', () => {
  it('Home shows linked computer and Send clipboard', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    const send = vi.spyOn(port, 'sendClipboard');
    expect(await screen.findByText('Desk PC')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: /Send clipboard/ }));
    const sheet = await screen.findByRole('dialog', { name: 'Send clipboard' });
    await user.click(within(sheet).getByRole('button', { name: 'Send' }));
    expect(send).toHaveBeenCalledWith('d2');
  });
});
