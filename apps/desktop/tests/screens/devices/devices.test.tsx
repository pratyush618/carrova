import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { AddDeviceDialog } from '@/screens/devices/AddDeviceDialog';
import { DevicePage } from '@/screens/devices/DevicePage';
import { LinkComputerDialog } from '@/screens/devices/LinkComputerDialog';
import { PhoneAppsSheet } from '@/screens/devices/PhoneAppsSheet';
import { deskPc, pixel, pixelInvite, pixelPeer } from '@/screens/devices/fixtures';
import type { DeviceIntents } from '@/screens/devices/model';
import type { LinkedDeviceView } from '@/engine/types';
import { revealDuration } from '@/ui/CodeDisplay/flaps';

const deviceOn = (): DeviceIntents => ({
  onFeature: vi.fn(),
  onRing: vi.fn(),
  onStopRing: vi.fn(),
  onSendClipboard: vi.fn(),
  onSendFiles: vi.fn(),
  onForget: vi.fn(),
  onRemind: vi.fn(),
  onApps: vi.fn(),
});

/** Pixel 8 giving notifications and messages here. */
const phone = {
  ...pixel,
  features: [
    ...pixel.features,
    {
      feature: 'notifications',
      group: 'quiet',
      local: 'on',
      remote: 'on',
      effective: 'on',
    },
    { feature: 'messages', group: 'quiet', local: 'on', remote: 'on', effective: 'on' },
  ],
} satisfies LinkedDeviceView;

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

  it('offers Stop ringing while the phone rings', async () => {
    const user = userEvent.setup();
    const on = deviceOn();
    renderScreen(<DevicePage device={pixel} ringing on={on} />);
    expect(screen.queryByRole('button', { name: 'Ring' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Stop ringing' }));
    expect(on.onStopRing).toHaveBeenCalledOnce();
    expect(on.onRing).not.toHaveBeenCalled();
  });

  it('dims the page and offers no actions while the device is away', () => {
    renderScreen(<DevicePage device={{ ...pixel, state: 'away' }} on={deviceOn()} />);
    expect(screen.getByText(/Pixel 8 is away/)).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Ring' })).not.toBeInTheDocument();
  });
});

describe('Moving with a trusted computer', () => {
  const move = (sendRequests: boolean) => ({
    sendRequests,
    onSendSetup: vi.fn(),
    onSendRequests: vi.fn(),
  });

  it('sends setup without a code and lets the user turn its requests off', async () => {
    const user = userEvent.setup();
    const moves = move(true);
    renderScreen(<DevicePage device={deskPc} move={moves} on={deviceOn()} />);
    expect(
      screen.getByText(
        'Desk PC can send you a move without a code. You still review it, and keys still ask.',
      ),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Send setup' }));
    expect(moves.onSendSetup).toHaveBeenCalledOnce();
    await user.click(screen.getByRole('switch', { name: 'Send requests' }));
    expect(moves.onSendRequests).toHaveBeenCalledWith(false);
  });

  it('says a computer with requests off needs a code', () => {
    renderScreen(<DevicePage device={deskPc} move={move(false)} on={deviceOn()} />);
    expect(screen.getByRole('switch', { name: 'Send requests' })).not.toBeChecked();
    expect(screen.getByText('Off: Desk PC needs a code to send you a move.')).toBeInTheDocument();
  });

  it('holds Send setup while the computer is away', () => {
    renderScreen(
      <DevicePage device={{ ...deskPc, state: 'away' }} move={move(true)} on={deviceOn()} />,
    );
    expect(screen.getByRole('button', { name: 'Send setup' })).toBeDisabled();
  });

  it('has no move group without code-less sends', () => {
    renderScreen(<DevicePage device={deskPc} on={deviceOn()} />);
    expect(screen.queryByText('Move your setup')).not.toBeInTheDocument();
  });
});

describe('Phone access and apps', () => {
  it('asks to remind on the phone for each access it lacks', async () => {
    const user = userEvent.setup();
    const on = deviceOn();
    renderScreen(
      <DevicePage device={{ ...phone, missing: ['notifications', 'messages'] }} on={on} />,
    );
    expect(screen.getByText('Notifications need access on Pixel 8')).toBeInTheDocument();
    expect(screen.getByText('Messages need SMS access on Pixel 8')).toBeInTheDocument();
    expect(screen.getAllByText('Waiting for access on the phone')).toHaveLength(2);
    expect(screen.queryByRole('button', { name: 'Apps' })).not.toBeInTheDocument();

    const [, messages] = screen.getAllByRole('button', { name: 'Remind on phone' });
    if (messages === undefined) throw new Error('no Messages reminder');
    await user.click(messages);
    expect(on.onRemind).toHaveBeenCalledWith('messages');
  });

  it("opens the phone's app list from the notifications row", async () => {
    const user = userEvent.setup();
    const on = deviceOn();
    renderScreen(<DevicePage device={phone} on={on} />);
    expect(screen.queryByRole('button', { name: 'Remind on phone' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Apps' }));
    expect(on.onApps).toHaveBeenCalledOnce();
  });

  it("shows the phone's app list read only", async () => {
    const user = userEvent.setup();
    const onClose = vi.fn();
    renderScreen(
      <PhoneAppsSheet
        name="Pixel 8"
        apps={[
          { id: 'org.bank', name: 'Bank of Example', on: false, private: true },
          { id: 'org.chat', name: 'Chat', on: true, private: false },
        ]}
        onClose={onClose}
      />,
    );
    expect(screen.getByText('Notifications from Pixel 8')).toBeInTheDocument();
    expect(screen.getByText('Bank of Example')).toBeInTheDocument();
    expect(screen.getByText('Off: banking apps start off')).toBeInTheDocument();
    expect(screen.getByText('Shows here')).toBeInTheDocument();
    expect(screen.queryByRole('switch')).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Done' }));
    expect(onClose).toHaveBeenCalledOnce();
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

  it('keeps focus in the dialog when Confirm is held', async () => {
    const user = userEvent.setup();
    const found = { state: 'found', peer: pixelPeer, answered: false } as const;
    const { rerender } = renderScreen(<AddDeviceDialog model={found} on={pairOn()} />);
    await user.click(screen.getByRole('button', { name: 'Confirm' }));
    rerender(<AddDeviceDialog model={{ ...found, answered: true }} on={pairOn()} />);
    const dialog = screen.getByRole('dialog', { name: 'Phone found' });
    expect(within(dialog).getByRole('status')).toHaveTextContent('Linking…');
    expect(dialog).toContainElement(
      document.activeElement instanceof HTMLElement ? document.activeElement : null,
    );
    expect(document.activeElement).not.toBe(document.body);
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
