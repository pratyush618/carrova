import { act, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { sendWait } from '@/screens/send/fixtures';
import type { WaitState } from '@/screens/send/model';
import { SendWaitDialog } from '@/screens/send/SendWaitDialog';

const intents = () => ({
  onCancel: vi.fn(),
  onClose: vi.fn(),
  onRetry: vi.fn(),
  onUseCode: vi.fn(),
});

const buttons = () => screen.getAllByRole('button').map((b) => b.textContent);

function show(state: WaitState) {
  const on = intents();
  renderScreen(<SendWaitDialog model={sendWait(state)} on={on} />);
  return on;
}

describe('waiting for a trusted computer', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('counts down the request’s 2 minutes with its counts on show', () => {
    vi.useFakeTimers({ now: 1_000_000 });
    show('waiting');
    expect(screen.getByRole('dialog', { name: 'Waiting for Desk PC' })).toBeInTheDocument();
    expect(screen.getByText(/12 apps · 340 MB · includes keys/)).toBeInTheDocument();
    expect(screen.getByText('1:38 left · the request expires after 2 minutes')).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(3_000);
    });
    expect(screen.getByText('1:35 left · the request expires after 2 minutes')).toBeInTheDocument();
    expect(buttons()).toEqual(['Cancel']);
  });

  it('lets the sender close while they review; the move carries on', async () => {
    const user = userEvent.setup();
    const on = show('reviewing');
    expect(buttons()).toEqual(['Cancel move', 'Close']);
    await user.click(screen.getByRole('button', { name: 'Close' }));
    expect(on.onClose).toHaveBeenCalledOnce();
    expect(on.onCancel).not.toHaveBeenCalled();
  });

  it('offers only "Change what moves" when declined', () => {
    show('declined');
    expect(screen.getByRole('alertdialog', { name: 'Desk PC declined' })).toBeInTheDocument();
    expect(buttons()).toEqual(['Close', 'Change what moves']);
  });

  it('offers "Send again" when it timed out', async () => {
    const user = userEvent.setup();
    const on = show('timeout');
    expect(screen.getByText('No answer from Desk PC')).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Send again' }));
    expect(on.onRetry).toHaveBeenCalledOnce();
  });

  it('offers "Try again" when it can’t be reached', () => {
    show('away');
    expect(screen.getByText("Can't reach Desk PC")).toBeInTheDocument();
    expect(buttons()).toEqual(['Close', 'Try again']);
  });

  it('says a silent refusal needs a code now, without saying why', async () => {
    const user = userEvent.setup();
    const on = show('needs_code');
    expect(screen.getByText('Desk PC needs a code now')).toBeInTheDocument();
    expect(screen.queryByText(/forgot|turned/i)).not.toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Use a code' }));
    expect(on.onUseCode).toHaveBeenCalledOnce();
  });
});
