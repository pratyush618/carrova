import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { SmsSendOutcome } from '@/bindings/SmsSendOutcome';
import type { ThreadRow } from '@/bindings/ThreadRow';
import type { ThreadView } from '@/bindings/ThreadView';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../../support/app';

/** A promise and the function that settles it. */
function deferred<T>() {
  let settle: (value: T) => void = () => undefined;
  const promise = new Promise<T>((resolve) => {
    settle = resolve;
  });
  return { promise, settle };
}

const page = (...texts: string[]): ThreadView => ({
  messages: texts.map((text, i) => ({
    id: 100 + i,
    outgoing: false,
    text,
    when_ms: i,
    provisional: false,
  })),
  next: null,
});

/** The linked fake sharing texts, its threads' by id: 1 is Sam's, 2 the unnamed number's. */
async function phonePort() {
  const port = createFakePort({ scenario: 'linked', stepMs: 0 });
  await port.setFeature('p8', 'messages', 'on');
  const thread = vi
    .spyOn(port, 'phoneThread')
    .mockImplementation((_device, id) =>
      Promise.resolve(id === 1 ? page('Hi from Sam') : page('Later, then')),
    );
  return { port, thread };
}

async function openMessages(user: UserEvent) {
  await screen.findByRole('heading', { level: 1, name: 'Good to see you again.' });
  await user.click(await screen.findByRole('button', { name: 'Messages' }));
  await screen.findByText('Hi from Sam');
}

const list = () => screen.getByRole('navigation', { name: 'Conversations' });

async function sendNew(user: UserEvent) {
  await user.click(screen.getByRole('button', { name: 'New message' }));
  await user.type(screen.getByRole('textbox', { name: 'To' }), '+15550177');
  await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'Hello');
  await user.click(screen.getByRole('button', { name: 'Send' }));
}

describe('Messages route', () => {
  it('shows no other conversation’s texts after a new message went', async () => {
    const user = userEvent.setup();
    const { port, thread } = await phonePort();
    await renderApp({ port, returning: true });
    await openMessages(user);
    await user.click(within(list()).getByRole('button', { name: /\+15550142/ }));
    await screen.findByText('Later, then');

    // Sam's conversation, opened once the text went, never answers.
    thread.mockImplementation(() => new Promise(() => undefined));
    await sendNew(user);
    await screen.findByRole('heading', { name: 'Sam Rivera' });
    expect(screen.queryByText('Later, then')).not.toBeInTheDocument();
  });

  it('keeps the conversation picked while a new message was on its way', async () => {
    const user = userEvent.setup();
    const { port } = await phonePort();
    const sent = deferred<SmsSendOutcome>();
    vi.spyOn(port, 'sendSms').mockReturnValue(sent.promise);
    await renderApp({ port, returning: true });
    await openMessages(user);
    await sendNew(user);
    await user.click(within(list()).getByRole('button', { name: /\+15550142/ }));
    await screen.findByText('Later, then');

    sent.settle('sent');
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByRole('heading', { name: '+15550142' })).toBeInTheDocument();
    expect(screen.getByText('Later, then')).toBeInTheDocument();
  });

  it('keeps a sent text shown until its stored copy has loaded', async () => {
    const user = userEvent.setup();
    const { port, thread } = await phonePort();
    await renderApp({ port, returning: true });
    await openMessages(user);
    const stored = deferred<ThreadView>();
    thread.mockReturnValue(stored.promise);

    await user.type(screen.getByRole('textbox', { name: 'Text message' }), 'On my way');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await new Promise((resolve) => setTimeout(resolve, 20));
    expect(screen.getByText('On my way')).toBeInTheDocument();

    stored.settle(page('Hi from Sam', 'On my way'));
    await vi.waitFor(() => {
      expect(screen.getAllByText('On my way')).toHaveLength(1);
    });
  });

  it('loads older conversations on asking, after the newest page', async () => {
    const user = userEvent.setup();
    const { port } = await phonePort();
    const first = await port.phoneThreads('p8', null, [], { now_ms: 0, utc_offset_min: 0 });
    const old: ThreadRow = {
      id: 3,
      name: 'Old friend',
      address: '+15550999',
      snippet: 'Long time',
      when_ms: 0,
      days_ago: 40,
      unread: 0,
      sim: null,
    };
    const threads = vi
      .spyOn(port, 'phoneThreads')
      .mockImplementation((_device, before) =>
        Promise.resolve(
          before === null
            ? { ...first, next: { when_ms: 1, id: 2 } }
            : { threads: [old], next: null },
        ),
      );
    await renderApp({ port, returning: true });
    await openMessages(user);
    expect(within(list()).queryByText('Old friend')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Show older conversations' }));
    expect(await within(list()).findByText('Old friend')).toBeInTheDocument();
    expect(threads).toHaveBeenLastCalledWith('p8', { when_ms: 1, id: 2 }, [], expect.anything());
    expect(
      screen.queryByRole('button', { name: 'Show older conversations' }),
    ).not.toBeInTheDocument();
  });

  it('loads older texts of the open conversation on asking', async () => {
    const user = userEvent.setup();
    const { port, thread } = await phonePort();
    thread.mockImplementation((_device, _id, before) =>
      Promise.resolve(
        before === null
          ? { ...page('Hi from Sam'), next: { when_ms: 0, id: 100 } }
          : page('Way back'),
      ),
    );
    await renderApp({ port, returning: true });
    await openMessages(user);
    expect(screen.queryByText('Way back')).not.toBeInTheDocument();

    await user.click(screen.getByRole('button', { name: 'Show older messages' }));
    expect(await screen.findByText('Way back')).toBeInTheDocument();
    expect(screen.getByText('Hi from Sam')).toBeInTheDocument();
    expect(thread).toHaveBeenLastCalledWith('p8', 1, { when_ms: 0, id: 100 });
    expect(screen.queryByRole('button', { name: 'Show older messages' })).not.toBeInTheDocument();
  });
});
