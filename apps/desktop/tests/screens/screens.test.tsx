import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../support/render';
import { RecvReviewScreen } from '@/screens/receive/RecvReviewScreen';
import { reviewDecide, reviewReady } from '@/screens/receive/fixtures';
import { OverrideDialog } from '@/screens/override/OverrideDialog';
import { OVERRIDES } from '@/screens/override/fixtures';
import { SendCodeScreen } from '@/screens/send/SendCodeScreen';
import { SendFoundScreen } from '@/screens/send/SendFoundScreen';
import { foundReady, pairingCantConnect, pairingLocked } from '@/screens/send/fixtures';

const reviewOn = () => ({
  onAnswer: vi.fn(),
  onApplyWhileOpen: vi.fn(),
  onIgnoreSpace: vi.fn(),
  onSeeEvery: vi.fn(),
  onCancel: vi.fn(),
  onApprove: vi.fn(),
});

describe('Here is what will change', () => {
  it('keeps Approve disabled and says how many answers are missing', () => {
    renderScreen(<RecvReviewScreen model={reviewDecide} on={reviewOn()} />);
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    expect(screen.getByText('Answer 3 questions to continue')).toBeInTheDocument();
    expect(screen.getAllByRole('radiogroup')).toHaveLength(3);
  });

  it('unlocks Approve once every decision has an answer', async () => {
    const on = reviewOn();
    renderScreen(<RecvReviewScreen model={reviewReady} on={on} />);
    await userEvent.click(screen.getByRole('button', { name: 'Approve and move' }));
    expect(on.onApprove).toHaveBeenCalledOnce();
    expect(screen.getByText('All answered')).toBeInTheDocument();
  });

  it('reports a decision with the item it belongs to', async () => {
    const on = reviewOn();
    renderScreen(<RecvReviewScreen model={reviewDecide} on={on} />);
    await userEvent.click(screen.getAllByRole('radio', { name: 'Bring them' })[0] ?? document.body);
    expect(on.onAnswer).toHaveBeenCalledWith('ssh/keys', 'bring');
  });
});

describe("Here's what we found", () => {
  it('starts sensitive items on Leave behind and notes that no keys move', () => {
    const on = {
      onToggleApp: vi.fn(),
      onAnswer: vi.fn(),
      onCustomize: vi.fn(),
      onPickFolders: vi.fn(),
      onSeeKnownApps: vi.fn(),
      onContinue: vi.fn(),
      onBack: vi.fn(),
    };
    renderScreen(<SendFoundScreen model={foundReady} purpose="send" on={on} />);
    const leave = screen.getAllByRole('radio', { name: 'Leave behind' });
    expect(leave.every((r) => r.getAttribute('aria-checked') === 'true')).toBe(true);
    expect(screen.getByText('1.2 GB, no keys')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Get pairing code' })).toBeEnabled();
  });
});

describe('Pairing code', () => {
  const on = { onBack: vi.fn(), onNewCode: vi.fn(), onHome: vi.fn() };

  it('shows the fallback address openly when discovery fails', () => {
    renderScreen(<SendCodeScreen model={pairingCantConnect} on={on} />, { reduceMotion: true });
    expect(screen.getByText('7-zebra-lemon@192.168.1.20:47822')).toBeVisible();
    expect(screen.queryByRole('status', { name: /Waiting/ })).not.toBeInTheDocument();
  });

  it('stops after repeated wrong codes with exactly one recovery', () => {
    renderScreen(<SendCodeScreen model={pairingLocked} on={on} />);
    expect(screen.getByRole('heading', { level: 1, name: 'Pairing stopped' })).toBeInTheDocument();
    expect(
      screen.getAllByRole('button').filter((b) => b.classList.contains('p-btn--primary')),
    ).toHaveLength(1);
  });
});

describe('Override dialog', () => {
  it('states the risk, names it on the button, and focuses the safe choice', () => {
    renderScreen(
      <OverrideDialog model={OVERRIDES.space} on={{ onKeepSafe: vi.fn(), onOverride: vi.fn() }} />,
    );
    expect(
      screen.getByRole('alertdialog', { name: 'Ignore the free-space check?' }),
    ).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Ignore and continue' })).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Keep it safe' })).toHaveFocus();
  });
});
