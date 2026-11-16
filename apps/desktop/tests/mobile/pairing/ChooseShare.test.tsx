import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone, scanToLinked } from '../../support/phone';

describe('Choose what to share asks for permissions', () => {
  it('turning_on_messages_asks_sms_perm', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const step = vi.spyOn(port, 'phonePermStep');
    const open = vi.spyOn(port, 'openPermSettings');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    expect(step).not.toHaveBeenCalled();

    await user.click(await screen.findByRole('switch', { name: 'Messages' }));

    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: 'Allow Carrova to send and view SMS messages?',
      }),
    ).toBeInTheDocument();
    expect(step).toHaveBeenCalledWith('sms');
    await user.click(screen.getByRole('button', { name: 'Open App info' }));
    expect(open).toHaveBeenCalledWith('sms');
    // Back keeps the choice made.
    await user.click(screen.getByRole('button', { name: 'Back' }));
    expect(await screen.findByRole('switch', { name: 'Messages' })).toBeChecked();
  });

  it('restricted_shows_app_info_button', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('restricted');
    const open = vi.spyOn(port, 'openPermSettings');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    const notifications = await screen.findByRole('switch', { name: 'Phone notifications' });
    await user.click(notifications);
    expect(screen.queryByRole('heading', { name: 'Allow restricted settings' })).toBeNull();

    await user.click(notifications);

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Allow restricted settings' }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Open App info' }));
    expect(open).toHaveBeenCalledWith('notification_access');
  });

  it('notification access opens Android settings for real', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const open = vi.spyOn(port, 'openPermSettings');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    const notifications = await screen.findByRole('switch', { name: 'Phone notifications' });
    await user.click(notifications);
    await user.click(notifications);

    await user.click(await screen.findByRole('button', { name: 'Open Android settings' }));
    expect(open).toHaveBeenCalledWith('notification_access');
  });

  it('a granted permission asks nothing', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    await user.click(await screen.findByRole('switch', { name: 'Calls' }));

    expect(screen.getByRole('switch', { name: 'Calls' })).toBeChecked();
    expect(screen.getByRole('heading', { level: 1, name: 'Choose what to share' })).toBeVisible();
  });

  it('words each feature as the phone sees it', async () => {
    const user = userEvent.setup();
    await renderPhone();
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));

    expect(
      await screen.findByText('From the computer by itself, to it with a tap'),
    ).toBeInTheDocument();
    expect(screen.queryByText(/this computer|here\.$/)).toBeNull();
  });
});
