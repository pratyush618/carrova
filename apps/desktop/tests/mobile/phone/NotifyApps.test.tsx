import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone } from '../../support/phone';
import { openScreen } from './support';

describe('Notifications on the computer', () => {
  it('private_app_off_by_default_in_list', async () => {
    const user = userEvent.setup();
    await renderPhone({ scenario: 'linked' });
    await openScreen(user, 'Notifications');

    expect(await screen.findByRole('switch', { name: 'Bank of Example' })).not.toBeChecked();
    expect(screen.getByText('Off by default for banking and password apps')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Chat' })).toBeChecked();
  });

  it('search narrows the list, and a switch saves the app', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    const set = vi.spyOn(port, 'setNotifyApp');
    await openScreen(user, 'Notifications');

    await user.type(await screen.findByRole('searchbox', { name: 'Search apps' }), 'ma');
    expect(screen.queryByRole('switch', { name: 'Chat' })).toBeNull();
    await user.click(screen.getByRole('switch', { name: 'Mail' }));
    expect(set).toHaveBeenCalledWith('org.mail', false);
    expect(screen.getByRole('switch', { name: 'Mail' })).not.toBeChecked();
  });

  it('hide codes and show text save together', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    const set = vi.spyOn(port, 'setNotifyFilters');
    await openScreen(user, 'Notifications');

    await user.click(await screen.findByRole('switch', { name: 'Show message text' }));
    expect(set).toHaveBeenCalledWith(true, false);
  });

  it('noaccess shows the way to Android settings', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    const open = vi.spyOn(port, 'openPermSettings');
    await openScreen(user, 'Notifications');

    expect(await screen.findByText('Notification access is off')).toBeInTheDocument();
    expect(screen.queryByRole('searchbox')).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Open Android settings' }));
    // The access step, then Android's page.
    await user.click(await screen.findByRole('button', { name: 'Open Android settings' }));
    expect(open).toHaveBeenCalledWith('notification_access');
  });
});
