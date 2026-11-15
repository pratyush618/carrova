import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderPhone, scanToLinked } from '../../support/phone';

describe('adding a computer from the phone', () => {
  it('Onboarding scan → linking → choose what to share shows Phase 1 rows only', async () => {
    const user = userEvent.setup();
    await renderPhone();
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Your computers, one tap away.' }),
    ).toBeInTheDocument();
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Choose what to share' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Shared clipboard' })).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Ring' })).toBeInTheDocument();
    expect(screen.getByRole('radiogroup', { name: 'Files' })).toBeInTheDocument();
    expect(screen.getByRole('radiogroup', { name: 'Handoff' })).toBeInTheDocument();
    expect(screen.queryByText(/Notifications|Messages|Calls|screen/)).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(await screen.findByText('Desk PC')).toBeInTheDocument();
  });

  it('Choose what to share preselects clipboard and ring', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone();
    await scanToLinked(user);
    await user.click(screen.getByRole('button', { name: 'Choose what to share' }));

    expect(await screen.findByRole('switch', { name: 'Shared clipboard' })).toBeChecked();
    expect(screen.getByRole('switch', { name: 'Ring' })).toBeChecked();
    await user.click(screen.getByRole('switch', { name: 'Ring' }));
    await user.click(screen.getByRole('button', { name: 'Done' }));
    await waitFor(async () => {
      const [desk] = await port.linkedDevices();
      const local = (feature: string) => desk?.features.find((f) => f.feature === feature)?.local;
      expect([local('clipboard'), local('ring')]).toEqual(['on', 'off']);
    });
  });

  it('Linking shows this phone’s own fingerprint', async () => {
    const user = userEvent.setup();
    // The fake's pairing never gets past its first step.
    await renderPhone({ stepMs: 1, sleep: () => new Promise<void>(() => undefined) });
    await user.click(await screen.findByRole('button', { name: 'Add a computer' }));
    await user.click(await screen.findByRole('button', { name: 'Scan the code' }));

    expect(await screen.findByRole('heading', { level: 1, name: 'Linking…' })).toBeInTheDocument();
    expect(await screen.findByText('This phone: A1B2·C3D4')).toBeInTheDocument();
    expect(screen.getByText('Confirm on your computer')).toBeInTheDocument();
  });

  it('Not on Wi-Fi state', async () => {
    const user = userEvent.setup();
    await renderPhone({ lan: false });
    await user.click(await screen.findByRole('button', { name: 'Add a computer' }));

    expect(
      await screen.findByRole('heading', { level: 1, name: 'Join the same Wi-Fi' }),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Scan the code' })).not.toBeInTheDocument();
  });

  it('asks for notification permission once a computer is linked', async () => {
    const user = userEvent.setup();
    const askNotifications = vi.fn(() => Promise.resolve());
    await renderPhone({}, { askNotifications });
    await screen.findByRole('button', { name: 'Add a computer' });
    expect(askNotifications).not.toHaveBeenCalled();

    await scanToLinked(user);
    expect(askNotifications).toHaveBeenCalledTimes(1);
  });

  it('still links when notifications are refused', async () => {
    const user = userEvent.setup();
    await renderPhone({}, { askNotifications: () => Promise.reject(new Error('denied')) });
    await scanToLinked(user);
    expect(screen.getByRole('button', { name: 'Choose what to share' })).toBeInTheDocument();
  });
});
