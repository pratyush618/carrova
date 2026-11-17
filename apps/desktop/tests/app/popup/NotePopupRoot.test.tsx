import { act, fireEvent, render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { NOTE_POPUP_TIMING, NotePopupRoot, notePopupTarget } from '@/app/popup/NotePopupRoot';
import type { CarrovaPort } from '@/engine/types';
import { createFakePort } from '@/ipc/fake';

const target = { device: 'p8', key: 'n1', name: 'Pixel 8' };

describe('Notification pop-up window', () => {
  it('reads its target from the query, never the content', () => {
    expect(notePopupTarget('?popup=note&device=p8&key=n1&name=Pixel+8')).toEqual(target);
    expect(notePopupTarget('?fake=linked')).toBeNull();
  });

  it('fetches the note by key, replies on the phone, and goes once done', async () => {
    const user = userEvent.setup();
    const port: CarrovaPort = createFakePort({ scenario: 'linked' });
    const action = vi.spyOn(port, 'phoneNoteAction');
    render(<NotePopupRoot port={port} target={target} />);
    expect(await screen.findByText('Are you still coming tonight?')).toBeInTheDocument();
    await user.type(screen.getByRole('textbox', { name: 'Reply' }), 'Yes');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    expect(action).toHaveBeenCalledWith('p8', { kind: 'reply', key: 'n1', text: 'Yes' });
    await vi.waitFor(() => {
      expect(screen.queryByText('Are you still coming tonight?')).not.toBeInTheDocument();
    });
  });

  it('says so when the notification is gone from the phone', async () => {
    const user = userEvent.setup();
    const port: CarrovaPort = createFakePort({ scenario: 'linked' });
    vi.spyOn(port, 'phoneNoteAction').mockResolvedValue('gone');
    render(<NotePopupRoot port={port} target={target} />);
    await user.click(await screen.findByRole('button', { name: 'Mark as read' }));
    expect(await screen.findByRole('alert')).toHaveTextContent(
      'This notification is gone from Pixel 8.',
    );
  });

  it('names the phone as it is linked now, not as the window was opened', async () => {
    const port: CarrovaPort = createFakePort({ scenario: 'linked' });
    render(<NotePopupRoot port={port} target={{ ...target, name: 'localhost' }} />);
    expect(await screen.findByText('Chat · Pixel 8')).toBeInTheDocument();
  });
});

describe('Notification pop-up auto-dismiss', () => {
  const { showMs, fadeMs } = NOTE_POPUP_TIMING;
  const text = 'Are you still coming tonight?';

  afterEach(() => {
    vi.useRealTimers();
    vi.unstubAllGlobals();
  });

  /** The pop-up on screen, with fake timers driving its clock. */
  async function shown() {
    vi.useFakeTimers();
    const port: CarrovaPort = createFakePort({ scenario: 'linked' });
    const action = vi.spyOn(port, 'phoneNoteAction');
    render(<NotePopupRoot port={port} target={target} />);
    // The fake port answers in microtasks: let them run without moving the clock.
    await act(() => vi.advanceTimersByTimeAsync(0));
    expect(screen.getByText(text)).toBeInTheDocument();
    return { action, popup: screen.getByRole('region') };
  }

  const advance = (ms: number) => {
    act(() => {
      vi.advanceTimersByTime(ms);
    });
  };

  it('fades out and closes after the timeout, leaving the note unread', async () => {
    const { action, popup } = await shown();
    advance(showMs - 100);
    expect(popup.parentElement).not.toHaveClass('is-out');
    advance(100);
    expect(popup.parentElement).toHaveClass('is-out');
    expect(screen.getByText(text)).toBeInTheDocument();
    advance(fadeMs);
    expect(screen.queryByText(text)).not.toBeInTheDocument();
    expect(action).not.toHaveBeenCalled();
  });

  it('waits while the pointer is over it, and starts again on leave', async () => {
    const { popup } = await shown();
    fireEvent.pointerEnter(popup);
    advance(showMs * 3);
    expect(screen.getByText(text)).toBeInTheDocument();
    fireEvent.pointerLeave(popup);
    advance(showMs - 100);
    expect(screen.getByText(text)).toBeInTheDocument();
    advance(100);
    advance(fadeMs);
    expect(screen.queryByText(text)).not.toBeInTheDocument();
  });

  it('waits while a reply is being typed', async () => {
    await shown();
    const reply = screen.getByRole('textbox', { name: 'Reply' });
    act(() => {
      reply.focus();
    });
    advance(showMs * 3);
    expect(screen.getByText(text)).toBeInTheDocument();
    act(() => {
      reply.blur();
    });
    advance(showMs);
    advance(fadeMs);
    expect(screen.queryByText(text)).not.toBeInTheDocument();
  });

  it('closes without a fade when motion is reduced', async () => {
    vi.stubGlobal('matchMedia', (query: string) => ({
      matches: query === '(prefers-reduced-motion: reduce)',
      addEventListener: () => undefined,
      removeEventListener: () => undefined,
    }));
    await shown();
    advance(showMs);
    expect(screen.queryByText(text)).not.toBeInTheDocument();
  });
});
