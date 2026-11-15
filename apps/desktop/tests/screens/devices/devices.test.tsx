import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { AddPhoneDialog } from '@/screens/devices/AddPhoneDialog';
import { DevicePage } from '@/screens/devices/DevicePage';
import { pixel, pixelInvite } from '@/screens/devices/fixtures';
import type { DeviceIntents } from '@/screens/devices/model';

const deviceOn = (): DeviceIntents => ({
  onFeature: vi.fn(),
  onRing: vi.fn(),
  onSendClipboard: vi.fn(),
  onSendFiles: vi.fn(),
  onForget: vi.fn(),
});

const pairOn = () => ({ onCancel: vi.fn(), onRetry: vi.fn(), onDone: vi.fn(), onOpen: vi.fn() });

describe('Device page', () => {
  it('DevicePage shows "Off on Pixel 8" when remote off', () => {
    renderScreen(<DevicePage device={pixel} on={deviceOn()} />);
    expect(screen.getByRole('heading', { level: 1, name: 'Pixel 8' })).toBeInTheDocument();
    expect(screen.getByText('Off on Pixel 8')).toBeInTheDocument();
    // Only Handoff is off on the phone: the line is said once.
    expect(screen.getAllByText(/Off on/)).toHaveLength(1);
  });

  it('changes a feature for this device only', async () => {
    const user = userEvent.setup();
    const on = deviceOn();
    renderScreen(<DevicePage device={pixel} on={on} />);
    await user.click(screen.getByRole('switch', { name: 'Shared clipboard' }));
    expect(on.onFeature).toHaveBeenCalledWith('clipboard', 'off');
    await user.click(
      within(screen.getByRole('radiogroup', { name: 'Files' })).getByRole('radio', { name: 'Off' }),
    );
    expect(on.onFeature).toHaveBeenCalledWith('files', 'off');
  });

  it('dims the page and offers no actions while the device is away', () => {
    renderScreen(<DevicePage device={{ ...pixel, state: 'away' }} on={deviceOn()} />);
    expect(screen.getByText(/Pixel 8 is away/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Ring' })).not.toBeInTheDocument();
  });
});

describe('Add a phone', () => {
  it('AddPhoneDialog renders QR svg and code, shows expired state', async () => {
    const user = userEvent.setup();
    const on = pairOn();
    const { rerender } = renderScreen(
      <AddPhoneDialog
        model={{ state: 'waiting', invite: pixelInvite, expiresAt: Date.now() + 160_000 }}
        on={on}
      />,
    );
    const qr = screen.getByRole('img', { name: 'Pairing QR code' });
    expect(qr.getAttribute('src')).toMatch(/^data:image\/svg\+xml,/);
    expect(screen.getByRole('img', { name: /4 maple orbit/ })).toBeInTheDocument();

    rerender(<AddPhoneDialog model={{ state: 'expired' }} on={on} />);
    expect(screen.getByRole('heading', { name: 'This code expired' })).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: 'Pairing QR code' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'New code' }));
    expect(on.onRetry).toHaveBeenCalledOnce();
  });

  it('names the linked phone and opens its page', async () => {
    const user = userEvent.setup();
    const on = pairOn();
    renderScreen(<AddPhoneDialog model={{ state: 'linked', name: 'Pixel 8' }} on={on} />);
    expect(screen.getByRole('heading', { name: 'Pixel 8 is linked' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Choose what it can do' }));
    expect(on.onOpen).toHaveBeenCalledOnce();
  });
});
