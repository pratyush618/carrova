import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { FeatureState, LinkedDeviceView } from '@/engine/types';
import { PickComputerSheet } from '@/mobile/home/PickComputerSheet';

/** A computer with clipboard `local` on this phone and `remote` there. */
const computer = (
  id: string,
  name: string,
  state: LinkedDeviceView['state'],
  local: FeatureState = 'on',
  remote: FeatureState | null = state === 'linked' ? 'on' : null,
): LinkedDeviceView => ({
  id,
  name,
  os: 'linux',
  fingerprint: 'B04E·88D1',
  state,
  features: [
    {
      feature: 'clipboard',
      group: 'quiet',
      local,
      remote,
      effective: local === 'off' || remote === null || remote === 'off' ? 'off' : 'on',
    },
  ],
  missing: [],
});

const sheet = (
  devices: readonly LinkedDeviceView[],
  on: { onSend?: (id: string) => void; onTurnOn?: (id: string) => void } = {},
) => (
  <PickComputerSheet
    title="Send text"
    feature="clipboard"
    devices={devices}
    onSend={on.onSend ?? vi.fn()}
    onTurnOn={on.onTurnOn ?? vi.fn()}
    onClose={vi.fn()}
  />
);

describe('picking the computer to send to', () => {
  it('lists every linked computer, with why the ones it cannot reach are out', () => {
    render(
      sheet([
        computer('d1', 'Desk PC', 'linked'),
        computer('d2', 'Laptop', 'away'),
        computer('d3', 'Studio', 'connecting'),
        computer('d4', 'Office', 'linked', 'on', 'off'),
      ]),
    );
    const dialog = screen.getByRole('dialog', { name: 'Send text' });
    expect(within(dialog).getByRole('radio', { name: 'Desk PC' })).toBeChecked();
    expect(within(dialog).queryByRole('radio', { name: 'Laptop' })).toBeNull();
    expect(within(dialog).getByText('Laptop')).toBeInTheDocument();
    expect(within(dialog).getByText('Away')).toBeInTheDocument();
    expect(within(dialog).getByText('Connecting…')).toBeInTheDocument();
    expect(within(dialog).getByText('Off on Office')).toBeInTheDocument();
    expect(within(dialog).queryByText('No computer nearby')).toBeNull();
  });

  it('offers to turn the feature on in place when it is off on this phone', async () => {
    const user = userEvent.setup();
    const onTurnOn = vi.fn();
    render(sheet([computer('d1', 'Desk PC', 'linked', 'off', 'on')], { onTurnOn }));
    expect(screen.getByText('Off on this phone')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    await user.click(screen.getByRole('button', { name: 'Turn on' }));
    expect(onTurnOn).toHaveBeenCalledWith('d1');
  });

  it('becomes selectable once the link connects (a share that started the app)', async () => {
    const user = userEvent.setup();
    const onSend = vi.fn();
    const { rerender } = render(sheet([computer('d1', 'Desk PC', 'connecting')], { onSend }));
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    rerender(sheet([computer('d1', 'Desk PC', 'linked')], { onSend }));
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(onSend).toHaveBeenCalledWith('d1');
  });

  it('says no computer is nearby only when none is linked at all', () => {
    render(sheet([]));
    expect(screen.getByText('No computer nearby')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
  });
});
