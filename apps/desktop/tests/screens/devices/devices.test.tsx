import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { AddDeviceDialog } from '@/screens/devices/AddDeviceDialog';
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

describe('Add a device', () => {
  it('AddDeviceDialog renders QR svg and code, shows expired state', async () => {
    const user = userEvent.setup();
    const on = pairOn();
    const { rerender } = renderScreen(
      <AddDeviceDialog
        model={{ state: 'waiting', invite: pixelInvite, expiresAt: Date.now() + 160_000 }}
        on={on}
      />,
    );
    const qr = screen.getByRole('img', { name: 'Pairing QR code' });
    expect(qr.getAttribute('src')).toMatch(/^data:image\/svg\+xml,/);
    expect(screen.getByRole('img', { name: /4 maple orbit/ })).toBeInTheDocument();

    rerender(<AddDeviceDialog model={{ state: 'expired' }} on={on} />);
    expect(screen.getByRole('heading', { name: 'This code expired' })).toBeInTheDocument();
    expect(screen.queryByRole('img', { name: 'Pairing QR code' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'New code' }));
    expect(on.onRetry).toHaveBeenCalledOnce();
  });

  it('offers the QR to a phone and the code to a computer', () => {
    renderScreen(
      <AddDeviceDialog
        model={{ state: 'waiting', invite: pixelInvite, expiresAt: Date.now() + 160_000 }}
        on={pairOn()}
      />,
    );
    expect(screen.getByRole('dialog', { name: 'Add a device' })).toBeInTheDocument();
    expect(screen.getByText(/On a phone, open Carrova/)).toBeInTheDocument();
    expect(
      screen.getByText('On a computer, choose Link a computer and type this code'),
    ).toBeInTheDocument();
  });

  it('names the linked phone and opens its page', async () => {
    const user = userEvent.setup();
    const on = pairOn();
    renderScreen(<AddDeviceDialog model={{ state: 'linked', device: pixel }} on={on} />);
    expect(screen.getByRole('heading', { name: 'Pixel 8 is linked' })).toBeInTheDocument();
    expect(
      screen.getByText(
        'Shared clipboard and Ring are on. Files and Handoff ask each time. Change any of it on its page.',
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Choose what it can do' }));
    expect(on.onOpen).toHaveBeenCalledOnce();
  });

  it('says what a device linked again kept, even when everything is off', () => {
    const off = {
      ...pixel,
      features: pixel.features.map((f) => ({ ...f, local: 'off' as const })),
    };
    renderScreen(<AddDeviceDialog model={{ state: 'linked', device: off }} on={pairOn()} />);
    expect(
      screen.getByText('Everything is off for now. Change any of it on its page.'),
    ).toBeInTheDocument();
  });
});
