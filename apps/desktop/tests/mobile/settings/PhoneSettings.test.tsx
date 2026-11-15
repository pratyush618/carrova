import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone } from '../../support/phone';

describe('the phone Settings', () => {
  it('updater hidden on android', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    const check = vi.spyOn(port, 'checkUpdate');
    await user.click(await screen.findByRole('button', { name: 'Settings' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Settings' })).toBeInTheDocument();
    expect(screen.queryByText(/update/i)).not.toBeInTheDocument();
    expect(check).not.toHaveBeenCalled();
  });
});
