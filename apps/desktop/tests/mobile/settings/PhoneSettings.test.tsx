import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../support/app';
import { renderPhone } from '../../support/phone';

describe('the phone Settings', () => {
  it('shows Updates: this version, Check now, and mobile data off by default', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    const check = vi.spyOn(port, 'checkUpdate');
    const save = vi.spyOn(port, 'saveSettings');
    await user.click(await screen.findByRole('button', { name: 'Settings' }));

    expect(await screen.findByRole('heading', { name: 'Updates' })).toBeInTheDocument();
    const mobile = screen.getByRole('switch', { name: 'Download on mobile data' });
    expect(mobile).not.toBeChecked();
    expect(screen.getByText('Off: updates wait for Wi-Fi')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Check now' }));
    expect(check).toHaveBeenCalled();
    await user.click(mobile);
    expect(save).toHaveBeenCalledWith({ app_update: { mobile_data: true } });
  });

  it('says why a check failed beside Check now, and lets the user try again', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    const check = vi
      .spyOn(port, 'checkUpdate')
      .mockRejectedValue({ code: 'network', message: "couldn't reach GitHub", hint: null });
    await user.click(await screen.findByRole('button', { name: 'Settings' }));

    await user.click(await screen.findByRole('button', { name: 'Check now' }));
    expect(await screen.findByText("couldn't reach GitHub")).toBeInTheDocument();
    const tries = check.mock.calls.length;
    await user.click(screen.getByRole('button', { name: 'Check now' }));
    expect(check).toHaveBeenCalledTimes(tries + 1);
  });

  it('offers a newer Carrova on Home and opens its page', async () => {
    const user = userEvent.setup();
    const port = createFakePort({ stepMs: 0, os: 'android', scenario: 'linked' });
    vi.spyOn(port, 'checkUpdate').mockResolvedValue({
      status: 'available',
      version: '0.4.0',
      current_version: '0.3.0',
      notes: 'Faster transfers',
      date: null,
    });
    await renderApp({ port });

    await user.click(await screen.findByRole('button', { name: 'View' }));
    expect(await screen.findByRole('heading', { level: 1, name: 'Update' })).toBeInTheDocument();
    expect(screen.getByText('Faster transfers')).toBeInTheDocument();
  });

  it('says once that the phone was updated', async () => {
    const port = createFakePort({ stepMs: 0, os: 'android', scenario: 'linked' });
    vi.spyOn(port, 'takeUpdated').mockResolvedValue('0.4.0');
    await renderApp({ port });
    expect(await screen.findByText('Updated to 0.4.0')).toBeInTheDocument();
  });
});
