import { act, screen } from '@testing-library/react';
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
});
