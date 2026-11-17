import { act, render, screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { App } from '@/app/App';
import type { CallView } from '@/bindings/CallView';
import type { SimChoice } from '@/bindings/SimChoice';
import type { BridgeEvent, CarrovaPort } from '@/engine/types';
import { LEFT_GRACE_MS } from '@/flows/calls/callTrack';
import { createFakePort } from '@/ipc/fake';

const ringing: CallView = {
  kind: 'ringing',
  device: 'p8',
  number: '+15550101',
  name: 'Sam Rivera',
  ring_here: true,
};
const active = (audio: 'phone' | 'laptop'): CallView => ({
  kind: 'active',
  device: 'p8',
  since_ms: Date.now(),
  muted: false,
  audio,
  outgoing: false,
});

/** The call window over the fake port, with the engine's call events in the test's hands. */
function renderCallWindow(now: CallView = { kind: 'none' }, sims: SimChoice[] = []) {
  const port: CarrovaPort = createFakePort({ stepMs: 0 });
  const listeners: ((event: BridgeEvent) => void)[] = [];
  vi.spyOn(port, 'subscribe').mockImplementation((listener) => {
    listeners.push(listener);
    return Promise.resolve(() => undefined);
  });
  vi.spyOn(port, 'callNow').mockResolvedValue(now);
  vi.spyOn(port, 'phoneSims').mockResolvedValue(sims);
  const phoneCall = vi.spyOn(port, 'phoneCall');
  const closeWindow = vi.fn(() => Promise.resolve());
  render(<App port={port} host={{ window: 'call', closeWindow }} />);
  const emit = (view: CallView) => {
    act(() => {
      for (const listener of listeners)
        listener({ kind: 'engine', session: 'link', event: { event: 'call_view', ...view } });
    });
  };
  return { emit, phoneCall, closeWindow };
}

describe('the call window', () => {
  let play: ReturnType<typeof vi.spyOn>;
  beforeEach(() => {
    play = vi.spyOn(window.HTMLMediaElement.prototype, 'play').mockResolvedValue();
    vi.spyOn(window.HTMLMediaElement.prototype, 'pause').mockImplementation(() => undefined);
  });
  afterEach(() => {
    vi.useRealTimers();
    vi.restoreAllMocks();
  });

  it('ringing_answer_calls_command', async () => {
    const user = userEvent.setup();
    const { phoneCall } = renderCallWindow(ringing);
    await user.click(await screen.findByRole('button', { name: 'Answer' }));
    expect(phoneCall).toHaveBeenCalledWith('p8', { kind: 'answer' });
    expect(screen.getByRole('heading', { name: 'Sam Rivera' })).toBeInTheDocument();
    expect(screen.getByText('SR')).toBeInTheDocument();
    expect(play).toHaveBeenCalledTimes(1);
  });

  it('shows a call placed from the phone as calling, with no timer', async () => {
    renderCallWindow({
      kind: 'active',
      device: 'p8',
      since_ms: Date.now() - 65_000,
      muted: false,
      audio: 'laptop',
      outgoing: true,
    });
    expect(await screen.findByText('Calling…')).toBeInTheDocument();
    expect(screen.queryByText(/\d\d:\d\d/)).not.toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Mute' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keypad' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'End' })).toBeInTheDocument();
  });

  it('left_state_closes_window', async () => {
    const { emit, closeWindow } = renderCallWindow(active('phone'));
    await screen.findByRole('button', { name: 'End' });
    expect(closeWindow).not.toHaveBeenCalled();
    emit({ kind: 'left', device: 'p8' });
    await waitFor(() => {
      expect(closeWindow).toHaveBeenCalled();
    });
  });

  it('keeps ringing through a reconnect: no close, no second ringtone', async () => {
    const { emit, closeWindow } = renderCallWindow(ringing);
    await screen.findByRole('button', { name: 'Answer' });
    vi.useFakeTimers();
    emit({ kind: 'left', device: 'p8' });
    act(() => {
      vi.advanceTimersByTime(LEFT_GRACE_MS / 2);
    });
    emit(ringing);
    act(() => {
      vi.advanceTimersByTime(LEFT_GRACE_MS * 2);
    });
    vi.useRealTimers();
    expect(closeWindow).not.toHaveBeenCalled();
    expect(screen.getByRole('button', { name: 'Answer' })).toBeInTheDocument();
    expect(play).toHaveBeenCalledTimes(1);
  });

  it('closes once a left while ringing outlasts the grace', async () => {
    const { emit, closeWindow } = renderCallWindow(ringing);
    await screen.findByRole('button', { name: 'Answer' });
    vi.useFakeTimers();
    emit({ kind: 'left', device: 'p8' });
    act(() => {
      vi.advanceTimersByTime(LEFT_GRACE_MS);
    });
    vi.useRealTimers();
    await waitFor(() => {
      expect(closeWindow).toHaveBeenCalled();
    });
  });

  it('keypad_only_with_laptop_audio', async () => {
    const user = userEvent.setup();
    const { emit, phoneCall } = renderCallWindow(active('phone'));
    await screen.findByRole('button', { name: 'End' });
    expect(screen.queryByRole('button', { name: 'Keypad' })).not.toBeInTheDocument();
    emit(active('laptop'));
    await user.click(await screen.findByRole('button', { name: 'Keypad' }));
    await user.click(screen.getByRole('button', { name: '5' }));
    expect(phoneCall).toHaveBeenCalledWith('p8', { kind: 'tones', digits: '5' });
  });

  it('keeps a missed call until closed, and calls back from the phone', async () => {
    const user = userEvent.setup();
    const { emit, phoneCall, closeWindow } = renderCallWindow(ringing);
    await screen.findByRole('button', { name: 'Answer' });
    emit({ kind: 'missed', device: 'p8', number: '+15550101', name: 'Sam Rivera' });
    emit({ kind: 'none' });
    await user.click(await screen.findByRole('button', { name: 'Call back' }));
    expect(phoneCall).toHaveBeenCalledWith('p8', { kind: 'dial', number: '+15550101', sim: null });
    expect(closeWindow).not.toHaveBeenCalled();
    // One SIM: no choice shown, the phone's default used.
    expect(screen.queryByRole('radiogroup', { name: 'Call from' })).not.toBeInTheDocument();
  });

  it('calls back from the SIM picked, the default preselected', async () => {
    const user = userEvent.setup();
    const sims: SimChoice[] = [
      { id: 3, slot: 1, name: 'Jio', default: false },
      { id: 7, slot: 2, name: 'airtel', default: true },
    ];
    const { phoneCall } = renderCallWindow(
      { kind: 'missed', device: 'p8', number: '+15550101', name: 'Sam Rivera' },
      sims,
    );
    expect(await screen.findByRole('radio', { name: 'SIM 2 · airtel' })).toBeChecked();
    await user.click(screen.getByRole('radio', { name: 'SIM 1 · Jio' }));
    await user.click(screen.getByRole('button', { name: 'Call back' }));
    expect(phoneCall).toHaveBeenCalledWith('p8', { kind: 'dial', number: '+15550101', sim: 3 });
  });
});
