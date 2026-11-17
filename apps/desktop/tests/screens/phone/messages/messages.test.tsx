import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen, testChrome } from '../../../support/render';
import type { SimChoice } from '@/bindings/SimChoice';
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
  sim: null,
};

const sims: SimChoice[] = [
  { id: 3, slot: 1, name: 'Jio', default: true },
  { id: 7, slot: 2, name: 'Airtel', default: false },
];

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
  sims: [],
};

const intents = (): MessagesIntents => ({
  onSelect: vi.fn(),
  onCompose: vi.fn(),
  onSend: vi.fn(),
  onRetry: vi.fn(),
  onOlderMessages: vi.fn(),
  onMoreThreads: vi.fn(),
});

/** The screen over the real send hook, as the route wires them. */
function Sending({
  sendSms,
  over = {},
}: {
  readonly sendSms: (...args: never[]) => Promise<SmsSendOutcome>;
  readonly over?: Partial<MessagesModel>;
}) {
  const { sends, send, retry } = useSmsSends({ sendSms }, 'p8', () => Promise.resolve());
  return (
    <MessagesScreen
      model={{ ...model, ...over, sends: sends.filter((s) => s.thread === sam.id) }}
      on={{
        ...intents(),
        onSend: (to, text, sim) => {
          send(to, text, sam.id, sim);
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
      .fn<
        (
          id: string,
          to: string,
          text: string,
          clientId: number,
          sim: number | null,
        ) => Promise<SmsSendOutcome>
      >()
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
    // One SIM: no choice shown, the phone's default used.
    expect(first?.[4]).toBeNull();
    expect(screen.queryByRole('radiogroup', { name: 'Send from' })).not.toBeInTheDocument();
  });

  it('replies from the SIM the conversation last used, or the one picked', async () => {
    const user = userEvent.setup();
    const sendSms = vi.fn(() => Promise.resolve<SmsSendOutcome>('unknown'));
    const thread = { ...sam, sim: 7 };
    renderScreen(
      <Sending sendSms={sendSms} over={{ sims, selected: thread, threads: [thread] }} />,
    );
    expect(screen.getByRole('radio', { name: 'SIM 2 · Airtel' })).toBeChecked();
    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'Hi');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(sendSms).toHaveBeenLastCalledWith('p8', '+15550101', 'Hi', expect.any(Number), 7);
    await user.click(screen.getByRole('radio', { name: 'SIM 1 · Jio' }));
    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'Again');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(sendSms).toHaveBeenLastCalledWith('p8', '+15550101', 'Again', expect.any(Number), 3);
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
    expect(on.onSend).toHaveBeenCalledWith('+15550177', 'Hi', null);
  });

  it('starts a new text from the default SIM on a dual-SIM phone', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(
      <MessagesScreen
        model={{ ...model, selected: null, thread: null, composing: true, sims }}
        on={on}
      />,
    );
    expect(screen.getByRole('radio', { name: 'SIM 1 · Jio' })).toBeChecked();
    await user.type(screen.getByRole('textbox', { name: 'To' }), '+15550177');
    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'Hi');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(on.onSend).toHaveBeenCalledWith('+15550177', 'Hi', 3);
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
          sends: [
            { clientId: 1, thread: null, to: '+15550101', text: 'Hi', sim: null, state: 'failed' },
          ],
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
    rerender(
      <ReduceMotionOverride value={false}>
        <ChromeContext value={testChrome}>
          <Window overlays={[]}>
            <MessagesScreen model={arrived('Running late?', 0, 50_000)} on={intents()} />
          </Window>
        </ChromeContext>
      </ReduceMotionOverride>,
    );
    expect(live()).toHaveTextContent('Running late?');
  });
});
