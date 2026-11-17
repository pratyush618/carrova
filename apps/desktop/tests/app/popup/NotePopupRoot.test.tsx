import { render, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { NotePopupRoot, notePopupTarget } from '@/app/popup/NotePopupRoot';
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

  it('acts once at a time: nothing more until the phone answered', async () => {
    const user = userEvent.setup();
    const port: CarrovaPort = createFakePort({ scenario: 'linked' });
    let answer: (outcome: 'failed') => void = () => undefined;
    const action = vi.spyOn(port, 'phoneNoteAction').mockImplementation(
      () =>
        new Promise((resolve) => {
          answer = resolve;
        }),
    );
    render(<NotePopupRoot port={port} target={target} />);
    await user.type(await screen.findByRole('textbox', { name: 'Reply' }), 'Yes');
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await user.click(screen.getByRole('button', { name: 'Send' }));
    await user.click(screen.getByRole('button', { name: 'Mark as read' }));
    expect(action).toHaveBeenCalledTimes(1);
    expect(screen.getByRole('button', { name: 'Send' })).toBeDisabled();

    answer('failed');
    await vi.waitFor(() => {
      expect(screen.getByRole('button', { name: 'Send' })).toBeEnabled();
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
});
