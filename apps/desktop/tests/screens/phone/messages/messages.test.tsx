import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { ReactElement } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen, testChrome } from '../../../support/render';
import type { SmsSendOutcome } from '@/bindings/SmsSendOutcome';
import type { ThreadRow } from '@/bindings/ThreadRow';
import { useSmsSends } from '@/flows/phone/useSmsSends';
import { ChromeContext, ReduceMotionOverride, Window } from '@/ui';
import {
  MessagesScreen,
  type MessagesIntents,
  type MessagesModel,
} from '@/screens/phone/messages/MessagesScreen';

const sam: ThreadRow = {
  id: 1,
  name: 'Sam Rivera',
  address: '+15550101',
  snippet: 'Are you still coming tonight?',
  when_ms: Date.now(),
  days_ago: 0,
  unread: 1,
};

const model: MessagesModel = {
  phone: 'Pixel 8',
  away: false,
  failed: false,
  threads: [sam],
  moreThreads: false,
  selected: sam,
  thread: {
    messages: [
      {
        id: 11,
        outgoing: false,
        text: 'Are you still coming tonight?',
        when_ms: 0,
        provisional: false,
      },
    ],
    next: null,
  },
  sends: [],
  composing: false,
};

const intents = (): MessagesIntents => ({
  onSelect: vi.fn(),
  onCompose: vi.fn(),
  onSend: vi.fn(),
  onRetry: vi.fn(),
  onOlderMessages: vi.fn(),
  onMoreThreads: vi.fn(),
});

/** `ui` in the chrome `renderScreen` wraps it in, for `rerender`. */
const chrome = (ui: ReactElement) => (
  <ReduceMotionOverride value={false}>
    <ChromeContext value={testChrome}>
      <Window overlays={[]}>{ui}</Window>
    </ChromeContext>
  </ReduceMotionOverride>
);

/** The screen over the real send hook, as the route wires them. */
function Sending({ sendSms }: { readonly sendSms: (...args: never[]) => Promise<SmsSendOutcome> }) {
  const { sends, send, retry } = useSmsSends({ sendSms }, 'p8', () => Promise.resolve());
  return (
    <MessagesScreen
      model={{ ...model, sends: sends.filter((s) => s.thread === sam.id) }}
      on={{
        ...intents(),
        onSend: (to, text) => {
          send(to, text, sam.id);
        },
        onRetry: retry,
      }}
    />
  );
}

describe('Messages', () => {
  it('lists conversations with the open one beside them', () => {
    renderScreen(<MessagesScreen model={model} on={intents()} />);
    expect(screen.getByRole('heading', { name: 'Sam Rivera' })).toBeInTheDocument();
    expect(screen.getByText('+15550101 · SMS via Pixel 8')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: /Sam Rivera/ })).toHaveAttribute(
      'aria-current',
      'true',
    );
  });

  it('thread_send_shows_failed_then_retry', async () => {
    const user = userEvent.setup();
    const sendSms = vi
      .fn<(id: string, to: string, text: string, clientId: number) => Promise<SmsSendOutcome>>()
      .mockResolvedValueOnce('no_signal')
      .mockResolvedValueOnce('sent');
    renderScreen(<Sending sendSms={sendSms} />);
    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'Running late');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(await screen.findByText('Not sent · Pixel 8 has no signal')).toBeInTheDocument();
    expect(screen.getByText('Running late')).toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Retry' }));
    await vi.waitFor(() => {
      expect(screen.queryByText('Running late')).not.toBeInTheDocument();
    });
    expect(sendSms).toHaveBeenCalledTimes(2);
    const [first, second] = sendSms.mock.calls;
    expect(first?.slice(0, 3)).toEqual(['p8', '+15550101', 'Running late']);
    // A retry is the same send: the phone sends at most once per client id.
    expect(second?.[3]).toBe(first?.[3]);
  });

  it('says an unknown send may have gone, with no one-click retry', async () => {
    const user = userEvent.setup();
    const sendSms = vi.fn(() => Promise.resolve<SmsSendOutcome>('unknown'));
    renderScreen(<Sending sendSms={sendSms} />);
    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'On my way');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(
      await screen.findByText('May have been sent · Check your phone before resending'),
    ).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Retry' })).not.toBeInTheDocument();
  });

  it('away_disables_send', () => {
    const on = intents();
    renderScreen(<MessagesScreen model={{ ...model, away: true }} on={on} />);
    expect(screen.getByText('Pixel 8 is away')).toBeInTheDocument();
    const field = screen.getByRole('textbox', { name: 'Text message' });
    expect(field).toBeDisabled();
    expect(field).toHaveAttribute('placeholder', 'Sending works when Pixel 8 is back');
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();
    // The texts loaded before it left stay readable.
    expect(screen.getAllByText('Are you still coming tonight?').length).toBeGreaterThan(0);
  });

  it('shows the empty state with a way to start a text', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(
      <MessagesScreen model={{ ...model, threads: [], selected: null, thread: null }} on={on} />,
    );
    expect(screen.getByText('Texts from Pixel 8 show up here')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'New message' }));
    expect(on.onCompose).toHaveBeenCalledOnce();
  });

  it('starts a new text to a typed number', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(
      <MessagesScreen
        model={{ ...model, selected: null, thread: null, composing: true }}
        on={on}
      />,
    );
    await user.type(screen.getByRole('textbox', { name: 'To' }), '+15550177');
    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'Hi');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(on.onSend).toHaveBeenCalledWith('+15550177', 'Hi');
  });

  it('shows how a new text went while the recipient matches a conversation', async () => {
    const user = userEvent.setup();
    renderScreen(
      <MessagesScreen
        model={{
          ...model,
          selected: null,
          thread: null,
          composing: true,
          sends: [{ clientId: 1, thread: null, to: '+15550101', text: 'Hi', state: 'failed' }],
        }}
        on={intents()}
      />,
    );
    await user.type(screen.getByRole('textbox', { name: 'To' }), '+1555');
    expect(screen.getAllByRole('button', { name: /Sam Rivera/ })).toHaveLength(2);
    expect(screen.getByRole('button', { name: 'Retry' })).toBeInTheDocument();
  });

  it('announces a text received while the conversation is open, not the ones loaded', () => {
    const live = () => document.querySelector('[aria-live="polite"]');
    const { rerender } = renderScreen(<MessagesScreen model={model} on={intents()} />);
    expect(live()).toHaveTextContent('');

    const arrived = (text: string, id: number, when_ms: number): MessagesModel => ({
      ...model,
      thread: {
        messages: [
          ...(model.thread?.messages ?? []),
          { id, outgoing: false, text, when_ms, provisional: id === 0 },
        ],
        next: null,
      },
    });
    rerender(chrome(<MessagesScreen model={arrived('Running late?', 0, 50_000)} on={intents()} />));
    expect(live()).toHaveTextContent('Running late?');
  });

  it('announces a stored text that repeats the one before it', () => {
    const live = () => document.querySelector('[aria-live="polite"]');
    const ok = (id: number, when_ms: number) => ({
      id,
      outgoing: false,
      text: 'OK',
      when_ms,
      provisional: false,
    });
    const first: MessagesModel = { ...model, thread: { messages: [ok(11, 0)], next: null } };
    const { rerender } = renderScreen(<MessagesScreen model={first} on={intents()} />);
    expect(live()).toHaveTextContent('');

    const again: MessagesModel = {
      ...model,
      thread: { messages: [ok(11, 0), ok(12, 30_000)], next: null },
    };
    rerender(chrome(<MessagesScreen model={again} on={intents()} />));
    expect(live()).toHaveTextContent('OK');
  });

  it('announces no received text an older page brings', () => {
    const live = () => document.querySelector('[aria-live="polite"]');
    const mine = { id: 21, outgoing: true, text: 'See you', when_ms: 9_000, provisional: false };
    const newest: MessagesModel = {
      ...model,
      thread: { messages: [mine], next: { when_ms: 9_000, id: 21 } },
    };
    const { rerender } = renderScreen(<MessagesScreen model={newest} on={intents()} />);

    const older: MessagesModel = {
      ...model,
      thread: {
        messages: [
          { id: 5, outgoing: false, text: 'Old news', when_ms: 0, provisional: false },
          mine,
        ],
        next: null,
      },
    };
    rerender(chrome(<MessagesScreen model={older} on={intents()} />));
    expect(screen.getByText('Old news')).toBeInTheDocument();
    expect(live()).toHaveTextContent('');
  });
});
