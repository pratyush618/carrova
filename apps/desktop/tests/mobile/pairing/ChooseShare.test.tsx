import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone, scanToLinked } from '../../support/phone';

describe('Choose what to share asks for permissions', () => {
  it('turning_on_messages_asks_android_for_sms', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const step = vi.spyOn(port, 'phonePermStep');
    const request = vi.spyOn(port, 'requestPhonePerm');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    expect(step).not.toHaveBeenCalledWith('sms');

    await user.click(await screen.findByRole('switch', { name: 'Messages' }));

    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: 'Allow Carrova to send and view SMS messages?',
      }),
    ).toBeInTheDocument();
    expect(step).toHaveBeenCalledWith('sms');
    expect(screen.queryByRole('button', { name: 'Open App info' })).toBeNull();
    await user.click(screen.getByRole('button', { name: 'Allow' }));
    expect(request).toHaveBeenCalledWith('sms');
    // Granted: the SMS step closes and contacts, the next one, is asked.
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Allow Carrova to see your contacts?' }),
    ).toBeInTheDocument();
    expect(screen.queryByRole('heading', { name: /SMS messages/ })).toBeNull();
  });

  it('a refused request offers App info', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    vi.spyOn(port, 'requestPhonePerm').mockResolvedValue('ask');
    const open = vi.spyOn(port, 'openPermSettings');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    await user.click(await screen.findByRole('switch', { name: 'Messages' }));

    await user.click(await screen.findByRole('button', { name: 'Allow' }));

    await user.click(await screen.findByRole('button', { name: 'Open App info' }));
    expect(open).toHaveBeenCalledWith('sms');
    expect(screen.getByText(/tap Permissions and allow it there/)).toBeVisible();
    // Back keeps the choice made.
    await user.click(screen.getByRole('button', { name: 'Back' }));
    expect(await screen.findByRole('switch', { name: 'Messages' })).toBeChecked();
  });

  it('calls then contacts are asked one after the other', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const request = vi.spyOn(port, 'requestPhonePerm');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    await user.click(await screen.findByRole('switch', { name: 'Calls' }));

    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: 'Allow Carrova to make and manage phone calls?',
      }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Allow' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Allow Carrova to see your contacts?' }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Allow' }));

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Choose what to share' }),
    ).toBeVisible();
    expect(screen.getByRole('switch', { name: 'Calls' })).toBeChecked();
    expect(request.mock.calls).toEqual([['calls'], ['contacts']]);
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

describe('Done asks for what the choices need', () => {
  it('Done with the defaults asks for notification access', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const step = vi.spyOn(port, 'phonePermStep');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));

    await user.click(await screen.findByRole('button', { name: 'Done' }));

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Notification access' }),
    ).toBeInTheDocument();
    expect(step).toHaveBeenCalledWith('notification_access');
  });

  it('Done with every phone feature off asks nothing', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    const step = vi.spyOn(port, 'phonePermStep');
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));
    await user.click(await screen.findByRole('switch', { name: 'Phone notifications' }));

    await user.click(screen.getByRole('button', { name: 'Done' }));

    expect(await screen.findByText('Desk PC')).toBeInTheDocument();
    expect(step).not.toHaveBeenCalledWith('notification_access');
  });
});

describe('Stay connected when closed', () => {
  it('Allow opens Android’s battery request and reads the answer back', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    let granted = false;
    vi.spyOn(port, 'phonePermStep').mockImplementation((perm) =>
      Promise.resolve(perm === 'battery' && granted ? 'ok' : 'ask'),
    );
    const open = vi.spyOn(port, 'openPermSettings').mockImplementation(() => {
      granted = true;
      return Promise.resolve();
    });
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));

    await user.click(await screen.findByRole('button', { name: 'Allow' }));
    expect(open).toHaveBeenCalledWith('battery');
    document.dispatchEvent(new Event('visibilitychange'));

    expect(await screen.findByText("Android won't stop Carrova in the background")).toBeVisible();
    expect(screen.queryByRole('button', { name: 'Allow' })).toBeNull();
  });
});
