import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../../support/render';
import type { NoteView } from '@/bindings/NoteView';
import { NotePopup, type NotePopupIntents } from '@/screens/phone/notes/NotePopup';

const note: NoteView = {
  key: 'n1',
  app_id: 'org.chat',
  app_name: 'Chat',
  title: 'Sam Rivera',
  text: 'Are you still coming tonight?',
  when_ms: 0,
  can_reply: true,
  icon: null,
  hidden: null,
};

const intents = (): NotePopupIntents => ({
  onReply: vi.fn(),
  onDismiss: vi.fn(),
  onOpen: vi.fn(),
  onClose: vi.fn(),
});

describe('Phone notification pop-up', () => {
  it('popup_reply_sends_action', async () => {
    const user = userEvent.setup();
    const on = intents();
    renderScreen(<NotePopup model={{ kind: 'note', phone: 'Pixel 8', note }} on={on} />);
    expect(screen.getByText('Chat · Pixel 8')).toBeInTheDocument();
    expect(screen.getByText('Are you still coming tonight?')).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Reply' }), 'Yes, leaving in 10');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(on.onReply).toHaveBeenCalledWith('Yes, leaving in 10');
    await user.click(screen.getByRole('button', { name: 'Mark as read' }));
    expect(on.onDismiss).toHaveBeenCalledOnce();
  });

  it('names the app by its label, never its package id', () => {
    renderScreen(
      <NotePopup
        model={{
          kind: 'note',
          phone: 'Pixel 8',
          note: { ...note, app_id: 'com.discord', app_name: 'Discord' },
        }}
        on={intents()}
      />,
    );
    expect(screen.getByText('Discord · Pixel 8')).toBeInTheDocument();
    expect(screen.queryByText(/com\.discord/)).not.toBeInTheDocument();
  });

  it('offers no reply where the app has none, and masks codes', () => {
    renderScreen(
      <NotePopup
        model={{
          kind: 'note',
          phone: 'Pixel 8',
          note: { ...note, can_reply: false, text: '', hidden: 'code' },
        }}
        on={intents()}
      />,
    );
    expect(screen.queryByRole('textbox', { name: 'Reply' })).not.toBeInTheDocument();
    expect(screen.getByText('One-time code, hidden')).toBeInTheDocument();
  });

  it('locked_shows_count_only', () => {
    renderScreen(
      <NotePopup model={{ kind: 'locked', phone: 'Pixel 8', count: 2 }} on={intents()} />,
    );
    expect(screen.getByText('Pixel 8: 2 new notifications')).toBeInTheDocument();
    expect(screen.getByText(/Unlock to see it/)).toBeInTheDocument();
    expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Mark as read' })).not.toBeInTheDocument();
  });
});
