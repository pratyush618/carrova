import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone } from '../../support/phone';
import { openScreen } from './support';

describe('Messages on the computer', () => {
  it('keep_in_sync_default_30_days', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    await openScreen(user, 'Messages');

    const keep = await screen.findByRole('radiogroup', { name: 'Keep in sync' });
    expect(await within(keep).findByRole('radio', { name: '30 days' })).toBeChecked();
    expect(await screen.findByText('Desk PC reads texts from the last 30 days.')).toBeVisible();
  });

  it('a new window is saved', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    await openScreen(user, 'Messages');

    await user.click(await screen.findByRole('radio', { name: 'All' }));
    expect(await port.messagesSync()).toBe('all');
    expect(screen.getByText('Desk PC reads all your texts.')).toBeVisible();
  });

  it('noperm asks for SMS from the banner', async () => {
    const user = userEvent.setup();
    await renderPhone({ scenario: 'linked' });
    await openScreen(user, 'Messages');

    expect(await screen.findByText("Carrova can't read texts yet")).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Allow' }));
    expect(
      await screen.findByRole('heading', {
        level: 1,
        name: 'Allow Carrova to send and view SMS messages?',
      }),
    ).toBeInTheDocument();
  });

  it('rcs note leads to the notification list', async () => {
    const user = userEvent.setup();
    await renderPhone({ scenario: 'linked' });
    await openScreen(user, 'Messages');

    expect(screen.getByText('Chat messages show as notifications')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Turn on notifications' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Notifications' }),
    ).toBeInTheDocument();
  });
});
