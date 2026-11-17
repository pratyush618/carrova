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

describe('Notifications list upkeep', () => {
  it('a switch that cannot be saved goes back and says so', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    vi.spyOn(port, 'setNotifyApp').mockRejectedValue(new Error('disk full'));
    await openScreen(user, 'Notifications');

    await user.click(await screen.findByRole('switch', { name: 'Chat' }));

    expect(await screen.findByText("Couldn't change that. Try again.")).toBeInTheDocument();
    expect(await screen.findByRole('switch', { name: 'Chat' })).toBeChecked();
  });

  it('the list loads again once access is granted', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    let granted = false;
    vi.spyOn(port, 'phonePermStep').mockImplementation(() =>
      Promise.resolve(granted ? 'ok' : 'open_settings'),
    );
    const list = vi.spyOn(port, 'notifyApps');
    await openScreen(user, 'Notifications');
    await screen.findByText('Notification access is off');
    const before = list.mock.calls.length;

    granted = true;
    document.dispatchEvent(new Event('visibilitychange'));

    expect(await screen.findByRole('searchbox', { name: 'Search apps' })).toBeInTheDocument();
    expect(list.mock.calls.length).toBeGreaterThan(before);
  });
});
