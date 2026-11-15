import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { AddDeviceDialog } from '@/screens/devices/AddDeviceDialog';
import { DevicePage } from '@/screens/devices/DevicePage';
import { LinkComputerDialog } from '@/screens/devices/LinkComputerDialog';
import { pixel, pixelInvite, pixelPeer } from '@/screens/devices/fixtures';
import type { DeviceIntents } from '@/screens/devices/model';
import { revealDuration } from '@/ui/CodeDisplay/flaps';

const deviceOn = (): DeviceIntents => ({
  onFeature: vi.fn(),
  onRing: vi.fn(),
  onSendClipboard: vi.fn(),
  onSendFiles: vi.fn(),
  onForget: vi.fn(),
});

const pairOn = () => ({
  onCancel: vi.fn(),
  onRetry: vi.fn(),
  onDone: vi.fn(),
  onOpen: vi.fn(),
  onConfirm: vi.fn(),
  onNotMine: vi.fn(),
});

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
  afterEach(() => {
    vi.useRealTimers();
  });

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

  it('flips the code in like the send screen, static under reduced motion', () => {
    vi.useFakeTimers({ toFake: ['setInterval', 'clearInterval', 'performance'] });
    const waiting = (
      <AddDeviceDialog
        model={{ state: 'waiting', invite: pixelInvite, expiresAt: Date.now() + 160_000 }}
        on={pairOn()}
      />
    );
    const tiles = () =>
      Array.from(screen.getByRole('img', { name: /4 maple orbit/ }).querySelectorAll('.p-flap'));
    const { unmount } = renderScreen(waiting);
    expect(tiles().every((t) => t.classList.contains('p-flap--spinning'))).toBe(true);
    act(() => {
      vi.advanceTimersByTime(revealDuration(tiles().length) + 120);
    });
    expect(
      tiles()
        .map((t) => t.textContent)
        .join(''),
    ).toBe('4mapleorbit');
    expect(tiles().every((t) => t.classList.contains('p-flap--set'))).toBe(true);
    unmount();

    renderScreen(waiting, { reduceMotion: true });
    expect(
      tiles()
        .map((t) => t.textContent)
        .join(''),
    ).toBe('4mapleorbit');
    expect(tiles().some((t) => t.classList.contains('p-flap--spinning'))).toBe(false);
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

  it('shows the phone found with its key, and Confirm confirms it', async () => {
    const user = userEvent.setup();
    const on = pairOn();
    renderScreen(
      <AddDeviceDialog model={{ state: 'found', peer: pixelPeer, answered: false }} on={on} />,
    );
    const found = screen.getByRole('dialog', { name: 'Phone found' });
    expect(within(found).getByText('Pixel 8 · Android')).toBeInTheDocument();
    expect(within(found).getByText('C4D2·7781')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Confirm' }));
    expect(on.onConfirm).toHaveBeenCalledOnce();
    expect(on.onNotMine).not.toHaveBeenCalled();
  });

  it("turns down a computer that is not the user's", async () => {
    const user = userEvent.setup();
    const on = pairOn();
    const desk = { ...pixelPeer, name: 'Desk PC', os: 'linux' as const };
    renderScreen(
      <AddDeviceDialog model={{ state: 'found', peer: desk, answered: false }} on={on} />,
    );
    expect(screen.getByRole('dialog', { name: 'Computer found' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Not my computer' }));
    expect(on.onNotMine).toHaveBeenCalledOnce();
    expect(on.onConfirm).not.toHaveBeenCalled();
  });

  it('holds both answers once the user confirmed, and shows Confirm working', () => {
    renderScreen(
      <AddDeviceDialog model={{ state: 'found', peer: pixelPeer, answered: true }} on={pairOn()} />,
    );
    const confirm = screen.getByRole('button', { name: 'Confirm' });
    expect(confirm).toBeDisabled();
    expect(confirm).toHaveAttribute('aria-busy', 'true');
    expect(screen.getByRole('button', { name: 'Not my phone' })).toBeDisabled();
  });

  it('says nothing linked after the user turned a computer down, and offers a new code', async () => {
    const user = userEvent.setup();
    const on = pairOn();
    renderScreen(<AddDeviceDialog model={{ state: 'rejected', kind: 'computer' }} on={on} />);
    const dialog = screen.getByRole('dialog', { name: 'Not linked' });
    expect(
      within(dialog).getByText(
        "You said this wasn't your computer. Nothing was saved on either side.",
      ),
    ).toBeInTheDocument();
    expect(within(dialog).queryByText(/Wi-Fi/)).not.toBeInTheDocument();
    await user.click(within(dialog).getByRole('button', { name: 'Try again' }));
    expect(on.onRetry).toHaveBeenCalledOnce();
  });
});

describe('Link a computer', () => {
  const linkOn = () => ({ onCode: vi.fn(), onLink: vi.fn(), onCancel: vi.fn(), onOpen: vi.fn() });

  it("shows this computer's own key while it links", () => {
    renderScreen(
      <LinkComputerDialog
        code="4-maple-orbit"
        state="linking"
        ownFingerprint="A1B2·C3D4"
        on={linkOn()}
      />,
    );
    expect(screen.getByText('This computer: A1B2·C3D4')).toBeInTheDocument();
    expect(screen.getByText('Confirm on your other computer')).toBeInTheDocument();
  });

  it('shows no key before linking starts', () => {
    renderScreen(
      <LinkComputerDialog code="" state="typing" ownFingerprint="A1B2·C3D4" on={linkOn()} />,
    );
    expect(screen.queryByText(/This computer:/)).not.toBeInTheDocument();
  });
});
