import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { BridgeEvent } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../support/app';

/** The fake `linked` port, with a way to push bridge events at the app. */
function linkedPort() {
  const port = createFakePort({ scenario: 'linked', stepMs: 0 });
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

const phoneNote: BridgeEvent = {
  kind: 'engine',
  session: 'link',
  event: {
    event: 'phone_note',
    device: 'p8',
    note: {
      key: 'n1',
      app_id: 'chat',
      app_name: 'Chat',
      title: 'Sam',
      text: 'hi',
      when_ms: 0,
      can_reply: false,
      icon: null,
      hidden: null,
      sms_app: false,
    },
  },
};

describe('Phone notification pop-ups', () => {
  it('pops a notification up once, not again after navigating', async () => {
    const user = userEvent.setup();
    const showNotePopup = vi.fn(() => Promise.resolve());
    const { port, push } = linkedPort();
    await renderApp({ port, returning: true, host: { showNotePopup } });
    await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
    push(phoneNote);
    await vi.waitFor(() => {
      expect(showNotePopup).toHaveBeenCalledTimes(1);
    });

    await user.click(screen.getByRole('button', { name: 'Nearby' }));
    await screen.findByRole('heading', { level: 1, name: 'Nearby' });
    expect(showNotePopup).toHaveBeenCalledTimes(1);
  });
});
