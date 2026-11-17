import { screen, within } from '@testing-library/react';
import userEvent, { type UserEvent } from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { ThreadView } from '@/bindings/ThreadView';
import { createFakePort } from '@/ipc/fake';
import { renderApp } from '../../../support/app';

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
});
