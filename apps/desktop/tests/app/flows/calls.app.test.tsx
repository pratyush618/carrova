import { act, screen, waitFor, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { CallView } from '@/bindings/CallView';
import type { BridgeEvent, CarrovaPort } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../support/app';

async function openSettings(user: UserEvent, section: string): Promise<void> {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
  await user.click(screen.getByRole('button', { name: 'Settings' }));
  await user.click(
    within(await screen.findByRole('navigation', { name: 'Settings sections' })).getByRole(
      'button',
      { name: section },
    ),
  );
}

/** A fake port whose call events the test sends. */
function callPort(): { port: CarrovaPort; emit: (view: CallView) => void } {
  const port = createFakePort({ stepMs: 0, scenario: 'linked' });
  const listeners: ((event: BridgeEvent) => void)[] = [];
  const subscribe = port.subscribe.bind(port);
  vi.spyOn(port, 'subscribe').mockImplementation((listener) => {
    listeners.push(listener);
    return subscribe(listener);
  });
  const emit = (view: CallView) => {
    act(() => {
      for (const listener of listeners)
        listener({ kind: 'engine', session: 'link', event: { event: 'call_view', ...view } });
    });
  };
  return { port, emit };
}

describe('calls on the laptop', () => {
  it('saves where calls ring from Settings › Calls and Bluetooth', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const save = vi.spyOn(port, 'setPhoneSettings');
    await openSettings(user, 'Calls and Bluetooth');
    await user.click(await screen.findByRole('radio', { name: 'Never' }));
    expect(save).toHaveBeenCalledWith(expect.objectContaining({ ring_here: 'never' }));
    await user.click(screen.getByRole('switch', { name: 'Only while I use this computer' }));
    expect(save).toHaveBeenLastCalledWith(
      expect.objectContaining({ ring_here: 'never', only_while_active: false }),
    );
  });

  it('shows the call on Home, ends it there, and says when the phone left', async () => {
    const user = userEvent.setup();
    const { port, emit } = callPort();
    const call = vi.spyOn(port, 'phoneCall');
    await renderApp({ returning: true, port });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    emit({
      kind: 'ringing',
      device: 'p8',
      number: '+15550101',
      name: 'Sam Rivera',
      ring_here: true,
    });
    emit({ kind: 'active', device: 'p8', since_ms: Date.now(), muted: false, audio: 'phone' });
    const bar = await screen.findByText(/Sam Rivera ·/);
    await waitFor(() => {
      expect(bar).toHaveTextContent(/^Sam Rivera · \d\d:\d\d · on Pixel 8$/);
    });
    await user.click(screen.getByRole('button', { name: 'End' }));
    expect(call).toHaveBeenCalledWith('p8', { kind: 'end' });
    emit({ kind: 'left', device: 'p8' });
    expect(await screen.findByText('Pixel 8 left')).toBeInTheDocument();
  });

  it('undoes the audio setup before setting up again, and turns it off', async () => {
    const user = userEvent.setup();
    const { port } = await renderApp({ returning: true });
    const config = vi.spyOn(port, 'setCallAudioConfig');
    await openSettings(user, 'Calls and Bluetooth');
    await user.click(await screen.findByRole('button', { name: 'Turn off' }));
    expect(config).toHaveBeenCalledWith(false);
    await user.click(screen.getByRole('button', { name: 'Set up again' }));
    expect(config).toHaveBeenCalledTimes(2);
    expect(config).toHaveBeenLastCalledWith(false);
    expect(
      await screen.findByRole('heading', { name: 'Answer calls on this computer' }),
    ).toBeInTheDocument();
  });
});
