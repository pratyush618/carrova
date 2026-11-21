import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { deskPcRequest } from '@/screens/home/fixtures';
import { EndedRequestNote, MoveRequestCard } from '@/screens/home/MoveRequestCard';

describe('a move request on Home', () => {
  it('names the computer with counts only and the time left', async () => {
    const user = userEvent.setup();
    const on = { onReview: vi.fn(), onDecline: vi.fn() };
    renderScreen(<MoveRequestCard request={deskPcRequest()} busy={false} on={on} />);
    expect(screen.getByText('Desk PC wants to send you its setup')).toBeInTheDocument();
    expect(screen.getByText(/^12 apps · 340 MB · includes keys · 1:4\d left$/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Review' }));
    await user.click(screen.getByRole('button', { name: 'Decline' }));
    expect(on.onReview).toHaveBeenCalledOnce();
    expect(on.onDecline).toHaveBeenCalledOnce();
  });

  it('holds Review while another move is received', () => {
    renderScreen(
      <MoveRequestCard
        request={deskPcRequest()}
        busy
        on={{ onReview: vi.fn(), onDecline: vi.fn() }}
      />,
    );
    expect(screen.getByRole('button', { name: 'Review' })).toBeDisabled();
    expect(screen.getByRole('button', { name: 'Decline' })).toBeEnabled();
    expect(screen.getByText(/Finish the current move first/)).toBeInTheDocument();
  });

  it('says an expired request changed nothing', async () => {
    const user = userEvent.setup();
    const onDismiss = vi.fn();
    renderScreen(
      <EndedRequestNote ended={{ name: 'Desk PC', why: 'expired' }} onDismiss={onDismiss} />,
    );
    expect(screen.getByText('A move request from Desk PC expired')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Dismiss' }));
    expect(onDismiss).toHaveBeenCalledOnce();
  });
});
