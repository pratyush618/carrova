import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { CarrovaPort } from '@/engine/types';
import { renderPhone } from '../../support/phone';
import { openScreen } from './support';

describe('Calls on the computer', () => {
  it('setup asks for calls, then contacts', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockImplementation((perm) =>
      Promise.resolve(perm === 'calls' ? 'ok' : 'ask'),
    );
    await openScreen(user, 'Calls');

    await user.click(await screen.findByRole('button', { name: 'Allow calls and contacts' }));
    expect(
      await screen.findByRole('heading', { level: 1, name: 'Allow Carrova to see your contacts?' }),
    ).toBeInTheDocument();
  });

  it('ready once both are granted', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    await callsOnBothSides(port);
    await openScreen(user, 'Calls');

    expect(await screen.findByText('Ready')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Allow calls and contacts' })).toBeNull();
    expect(screen.getByText("Use Desk PC's mic and speakers")).toBeInTheDocument();
  });

  it('not ready while calls are off for the computer', async () => {
    const user = userEvent.setup();
    const { port } = await renderPhone({ scenario: 'linked' });
    vi.spyOn(port, 'phonePermStep').mockResolvedValue('ok');
    await openScreen(user, 'Calls');

    expect(await screen.findByText('Calls are off')).toBeInTheDocument();
    expect(screen.queryByText('Ready')).toBeNull();
  });
});

/** Turn calls on for Desk PC on both sides (the fake's computer has them off). */
async function callsOnBothSides(port: CarrovaPort) {
  const devices = await port.linkedDevices();
  vi.spyOn(port, 'linkedDevices').mockResolvedValue(
    devices.map((d) => ({
      ...d,
      features: d.features.map((f) =>
        f.feature === 'calls' ? { ...f, local: 'on', remote: 'on', effective: 'on' } : f,
      ),
    })),
  );
  await act(() => port.setFeature('d2', 'calls', 'on'));
}
