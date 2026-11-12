import { act, screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { BridgeEvent } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../support/app';

/** The fake `nearby` port, with a way to push bridge events at the app. */
function nearbyPort() {
  const port = createFakePort({ scenario: 'nearby', stepMs: 0 });
  const listeners: ((event: BridgeEvent) => void)[] = [];
  const subscribe = port.subscribe.bind(port);
  port.subscribe = (listener) => {
    listeners.push(listener);
    return subscribe(listener);
  };
  const push = (event: BridgeEvent) => {
    act(() => {
      for (const listener of listeners) listener(event);
    });
  };
  return { port, push };
}

async function openNearby(user: UserEvent): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
  await user.click(screen.getByRole('button', { name: 'Nearby' }));
  await screen.findByRole('heading', { level: 1, name: 'Nearby' });
}

describe('nearby', () => {
  it('pill and settings bind one value', async () => {
    const user = userEvent.setup();
    await renderApp({ scenario: 'nearby', returning: true });
    await openNearby(user);
    await user.click(await screen.findByRole('button', { name: /Visible to known devices/ }));
    await user.click(screen.getByRole('button', { name: /^Everyone for 10 minutes/ }));
    await screen.findByRole('button', { name: /Visible to everyone/ });

    await user.click(screen.getByRole('button', { name: /Visible to everyone/ }));
    await user.click(screen.getByRole('button', { name: /^Nearby settings/ }));
    const visibility = await screen.findByRole('radiogroup', { name: 'Visibility' });
    expect(within(visibility).getByRole('radio', { name: 'Everyone · 10 min' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.click(within(visibility).getByRole('radio', { name: 'No one' }));
    await user.click(screen.getByRole('button', { name: 'Back from settings' }));
    expect(await screen.findByRole('button', { name: /Visible to no one/ })).toBeInTheDocument();
    // Hidden from everyone: the sidebar's dot goes out.
    const link = within(screen.getByRole('navigation', { name: 'Main' })).getByRole('button', {
      name: 'Nearby',
    });
    expect(link.querySelector('.p-side__live')).toBeNull();
  });

  it('wakes an away device with a hint, and says when one has none', async () => {
    const user = userEvent.setup();
    const { port } = nearbyPort();
    const wake = vi.spyOn(port, 'wakeDevice');
    await renderApp({ port, returning: true });
    await openNearby(user);
    await user.click(screen.getByRole('radio', { name: 'List' }));
    await user.click(await screen.findByRole('button', { name: 'Wake: Old ThinkPad' }));
    expect(wake).toHaveBeenCalledWith('o3');
    expect(await screen.findByText(/Wake sent to Old ThinkPad/)).toBeInTheDocument();
  });

  it('remembered notice offers forget', async () => {
    const user = userEvent.setup();
    const { port, push } = nearbyPort();
    const forget = vi.spyOn(port, 'forgetDevice');
    await renderApp({ port, returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    push({
      kind: 'engine',
      session: 's1',
      event: { event: 'device_remembered', name: "Ana's laptop", fingerprint: '7F3A·19C2' },
    });
    const text = await screen.findByText("Remembered Ana's laptop (7F3A·19C2)");
    const notice = text.closest<HTMLElement>('[role=status]');
    if (notice === null) throw new Error('the notice is a status');
    await user.click(within(notice).getByRole('button', { name: 'Forget' }));
    expect(forget).toHaveBeenCalledWith({ device: '7F3A·19C2' });
    expect(screen.queryByText(/Remembered Ana's laptop/)).not.toBeInTheDocument();
    // Closed stays closed across navigation.
    await user.click(screen.getByRole('button', { name: 'History' }));
    expect(screen.queryByText(/Remembered Ana's laptop/)).not.toBeInTheDocument();
  });

  it("opens Nearby when the tray's Nearby is chosen", async () => {
    const { port, push } = nearbyPort();
    await renderApp({ port, returning: true });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    vi.spyOn(port, 'takeShowNearby').mockResolvedValueOnce(true);
    push({ kind: 'show_nearby' });
    expect(await screen.findByRole('heading', { level: 1, name: 'Nearby' })).toBeInTheDocument();
  });

  it('opens Nearby when the tray asked before the window was listening', async () => {
    const { port } = nearbyPort();
    vi.spyOn(port, 'takeShowNearby').mockResolvedValueOnce(true);
    await renderApp({ port, returning: true });
    expect(await screen.findByRole('heading', { level: 1, name: 'Nearby' })).toBeInTheDocument();
  });
});
