import { act, screen } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { clock, minutesLeft, secondsLeft } from '@/screens/send/countdown';
import { move } from '@/screens/send/fixtures';
import type { PairingModel } from '@/screens/send/model';
import { SendCodeScreen } from '@/screens/send/SendCodeScreen';

const on = { onBack: vi.fn(), onNewCode: vi.fn(), onHome: vi.fn() };
const NOW = 1_000_000;
const code = (status: 'waiting' | 'expired'): PairingModel => ({
  status,
  code: ['7', 'acid', 'lemon'],
  address: null,
  expiresAt: NOW + 292_000,
});

describe('code countdown', () => {
  it('counts whole seconds down to zero and reads as m:ss', () => {
    expect(secondsLeft(NOW + 292_000, NOW)).toBe(292);
    expect(secondsLeft(NOW + 1, NOW)).toBe(1);
    expect(secondsLeft(NOW - 5_000, NOW)).toBe(0);
    expect(clock(292)).toBe('4:52');
    expect(clock(5)).toBe('0:05');
  });

  it('announces whole minutes only, then "under a minute"', () => {
    expect(minutesLeft(292)).toBe(5);
    expect(minutesLeft(241)).toBe(5);
    expect(minutesLeft(240)).toBe(4);
    expect(minutesLeft(60)).toBe(0);
  });
});

describe('Pairing code', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('shows when the code changes, ticking each second but announcing per minute', () => {
    vi.useFakeTimers({ now: NOW });
    renderScreen(<SendCodeScreen model={code('waiting')} move={move} on={on} />, {
      reduceMotion: true,
    });
    expect(screen.getByText('New code in 4:52')).toBeInTheDocument();
    expect(screen.getByText('This code changes in 5 minutes')).toBeInTheDocument();
    act(() => {
      vi.advanceTimersByTime(2_000);
    });
    expect(screen.getByText('New code in 4:50')).toBeInTheDocument();
    expect(screen.getByText('This code changes in 5 minutes')).toBeInTheDocument();
    expect(screen.queryByText(/previous code expired/)).not.toBeInTheDocument();
  });

  it('says the previous code expired when the engine replaced it', () => {
    renderScreen(<SendCodeScreen model={code('expired')} move={move} on={on} />, {
      reduceMotion: true,
    });
    expect(
      screen.getByText('The previous code expired, so here is a new one.'),
    ).toBeInTheDocument();
  });

  it('says the code is being checked once the other computer typed it', () => {
    renderScreen(<SendCodeScreen model={{ status: 'checking' }} move={move} on={on} />);
    expect(screen.getByRole('heading', { level: 1, name: 'Checking the code…' })).toBeVisible();
    expect(screen.getByRole('status')).toHaveTextContent('Someone typed the code.');
  });
});
