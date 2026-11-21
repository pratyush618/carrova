import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../support/app';
import { renderPhone } from '../../support/phone';

describe('the phone Welcome', () => {
  it('checks for an app update before any computer is linked', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const check = vi.spyOn(port, 'checkUpdate');
    const link = await screen.findByRole('button', { name: 'Check for app updates' });
    const tries = check.mock.calls.length;
    await user.click(link);

    expect(check).toHaveBeenCalledTimes(tries + 1);
    expect(await screen.findByRole('heading', { level: 1, name: 'Update' })).toBeInTheDocument();
  });

  it('opens the update on offer', async () => {
    const user = userEvent.setup();
    const port = createFakePort({ stepMs: 0, os: 'android' });
    vi.spyOn(port, 'checkUpdate').mockResolvedValue({
      status: 'available',
      version: '0.4.0',
      current_version: '0.3.0',
      notes: 'Faster transfers',
      date: null,
    });
    await renderApp({ port });

    await user.click(await screen.findByRole('button', { name: 'Carrova 0.4.0 is ready' }));
    expect(await screen.findByText('Faster transfers')).toBeInTheDocument();
  });
});
