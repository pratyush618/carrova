import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../support/render';
import { RecvCodeScreen } from '@/screens/receive/RecvCodeScreen';
import { codeConnecting } from '@/screens/receive/fixtures';
import { RecvReviewScreen } from '@/screens/receive/RecvReviewScreen';
import { PluginConsentDialog } from '@/screens/receive/PluginConsentDialog';
import { RecvProgressScreen } from '@/screens/receive/RecvProgressScreen';
import {
  pluginConsent,
  progressAt,
  reviewCrossOs,
  reviewDecide,
  reviewReady,
} from '@/screens/receive/fixtures';
import { OverrideDialog } from '@/screens/override/OverrideDialog';
import { OVERRIDES } from '@/screens/override/fixtures';
import { KeyConfirmDialog } from '@/screens/send/KeyConfirmDialog';
import { SendCodeScreen } from '@/screens/send/SendCodeScreen';
import { SendFoundScreen } from '@/screens/send/SendFoundScreen';
import { SendLinkedScreen } from '@/screens/send/SendLinkedScreen';
import {
  foundReady,
  HERE,
  linkedCrossOs,
  linkedSameOs,
  move,
  pairingCantConnect,
  pairingLocked,
  SECRETS,
} from '@/screens/send/fixtures';

const reviewOn = () => ({
  onAnswer: vi.fn(),
  onApplyWhileOpen: vi.fn(),
  onIgnoreSpace: vi.fn(),
  onBringAnyway: vi.fn(),
  onReviewPlugin: vi.fn(),
  onSeeWhere: vi.fn(),
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
    expect(screen.getByText('All set')).toBeInTheDocument();
  });

  it('reports a decision with the item it belongs to', async () => {
    const on = reviewOn();
    renderScreen(<RecvReviewScreen model={reviewDecide} on={on} />);
    await userEvent.click(screen.getAllByRole('radio', { name: 'Bring them' })[0] ?? document.body);
    expect(on.onAnswer).toHaveBeenCalledWith('ssh/keys', 'bring');
  });

  it('asks about an item from another system: leave it, or override behind a warning', async () => {
    const on = reviewOn();
    renderScreen(<RecvReviewScreen model={reviewCrossOs} on={on} />);
    expect(screen.getByText('GNOME Terminal profile only works on macOS')).toBeInTheDocument();
    await userEvent.click(
      screen.getByRole('button', { name: 'Leave behind (GNOME Terminal profile)' }),
    );
    expect(on.onAnswer).toHaveBeenCalledWith('gnome-terminal/profile', 'leave');
    await userEvent.click(
      screen.getByRole('button', { name: 'Override… (GNOME Terminal profile)' }),
    );
    expect(on.onBringAnyway).toHaveBeenCalledWith('gnome-terminal/profile');
  });

  it('opens a plugin’s permissions from its question and where paths land from the suggestion', async () => {
    const on = reviewOn();
    renderScreen(<RecvReviewScreen model={reviewCrossOs} on={on} />);
    expect(screen.getByText('The path-rewrite plugin would change Vault list')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Approve and move' })).toBeDisabled();
    await userEvent.click(screen.getByRole('button', { name: 'Review… (path-rewrite plugin)' }));
    expect(on.onReviewPlugin).toHaveBeenCalledWith('path-rewrite');
    expect(screen.getByText('macOS paths become Linux paths')).toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'See where' }));
    expect(on.onSeeWhere).toHaveBeenCalledOnce();
  });
});

describe('Plugin consent', () => {
  it('lists what the plugin may do and allows it for this move only', async () => {
    const on = { onAllow: vi.fn(), onDecline: vi.fn(), onClose: vi.fn() };
    renderScreen(<PluginConsentDialog model={pluginConsent} on={on} />);
    expect(screen.getByRole('dialog', { name: 'Allow “path-rewrite”?' })).toBeInTheDocument();
    expect(screen.getByText('It cannot use the network.')).toBeInTheDocument();
    expect(screen.queryByRole('checkbox')).not.toBeInTheDocument();
    await userEvent.click(screen.getByRole('button', { name: 'Allow for this move' }));
    expect(on.onAllow).toHaveBeenCalledOnce();
  });
});

describe("Here's what we found", () => {
  const foundOn = () => ({
    onToggleApp: vi.fn(),
    onConfirmKey: vi.fn(),
    onLeaveKey: vi.fn(),
    onFilter: vi.fn(),
    onPreset: vi.fn(),
    onCustomize: vi.fn(),
    onPickFolders: vi.fn(),
    onSeeKnownApps: vi.fn(),
    onContinue: vi.fn(),
    onBack: vi.fn(),
  });

  it('puts keys and secrets first, unticked, and notes that no keys move', () => {
    renderScreen(<SendFoundScreen model={foundReady} purpose="send" here={HERE} on={foundOn()} />);
    const secrets = screen.getByRole('list', { name: 'Keys and secrets' });
    const apps = screen.getByRole('list', { name: 'Developer tools' });
    expect(secrets.compareDocumentPosition(apps) & Node.DOCUMENT_POSITION_FOLLOWING).toBeTruthy();
    const keys = within(secrets).getAllByRole('checkbox');
    expect(keys.every((k) => k.getAttribute('aria-checked') === 'false')).toBe(true);
    expect(screen.getByText('4 apps · 1.2 GB · no keys')).toBeInTheDocument();
    expect(screen.getByRole('button', { name: 'Get pairing code' })).toBeEnabled();
  });

  it('asks before bringing a secret, from its box or its Include button', async () => {
    const user = userEvent.setup();
    const on = foundOn();
    renderScreen(<SendFoundScreen model={foundReady} purpose="send" here={HERE} on={on} />);
    await user.click(screen.getByRole('checkbox', { name: 'Bring SSH key pairs (private keys)' }));
    await user.click(screen.getByRole('button', { name: 'Include GnuPG secret keys…' }));
    expect(on.onConfirmKey.mock.calls).toEqual([['ssh/keys'], ['gpg/secret-keys']]);
  });

  it('picks a whole category from its heading box, which shows mixed when partly picked', async () => {
    const user = userEvent.setup();
    const on = foundOn();
    renderScreen(<SendFoundScreen model={foundReady} purpose="send" here={HERE} on={on} />);
    const all = screen.getByRole('checkbox', { name: 'Bring every app in Security' });
    expect(all).toHaveAttribute('aria-checked', 'mixed');
    await user.click(all);
    expect(on.onToggleApp.mock.calls).toEqual([
      ['ssh', true],
      ['gpg', true],
    ]);
  });
});

describe('Key confirm', () => {
  it('lists the key files and focuses Leave behind', () => {
    const [secret] = SECRETS;
    if (secret === undefined) throw new Error('fixture');
    renderScreen(<KeyConfirmDialog secret={secret} on={{ onBring: vi.fn(), onLeave: vi.fn() }} />);
    const dialog = screen.getByRole('alertdialog', { name: 'Bring SSH key pairs (private keys)?' });
    expect(within(dialog).getByText('~/.ssh/id_ed25519_work')).toBeInTheDocument();
    expect(within(dialog).getByRole('button', { name: 'Leave behind' })).toHaveFocus();
  });
});

describe('Pairing code', () => {
  const on = { onBack: vi.fn(), onNewCode: vi.fn(), onHome: vi.fn() };

  it('shows the fallback address openly when discovery fails', () => {
    renderScreen(<SendCodeScreen model={pairingCantConnect} move={move} on={on} />, {
      reduceMotion: true,
    });
    expect(screen.getByText('7-zebra-lemon@192.168.1.20:47822')).toBeVisible();
    expect(screen.queryByRole('status', { name: /Waiting/ })).not.toBeInTheDocument();
  });

  it('stops after repeated wrong codes with exactly one recovery', () => {
    renderScreen(<SendCodeScreen model={pairingLocked} move={move} on={on} />);
    expect(screen.getByRole('heading', { level: 1, name: 'Pairing stopped' })).toBeInTheDocument();
    expect(
      screen.getAllByRole('button').filter((b) => b.classList.contains('p-btn--primary')),
    ).toHaveLength(1);
  });
});

describe('Receiver handshake', () => {
  it("shows the code both sides typed and this computer's OS while checking", () => {
    const on = {
      onChange: vi.fn(),
      onAddress: vi.fn(),
      onConnect: vi.fn(),
      onCancel: vi.fn(),
      onBack: vi.fn(),
    };
    renderScreen(
      <RecvCodeScreen model={codeConnecting} here={{ name: 'ana-laptop', os: 'linux' }} on={on} />,
      { reduceMotion: true },
    );
    expect(screen.getByRole('img', { name: 'Pairing code: 7 zebra lemon' })).toBeInTheDocument();
    expect(screen.getByText('The code both sides typed')).toBeInTheDocument();
    expect(screen.getAllByText('Linux').length).toBeGreaterThan(0);
  });
});

describe('Linked', () => {
  it('says when the move crosses to another OS, and only then', () => {
    const on = { onCancel: vi.fn() };
    const { unmount } = renderScreen(<SendLinkedScreen model={linkedCrossOs} on={on} />);
    expect(screen.getByText('Moving across systems')).toBeInTheDocument();
    unmount();
    renderScreen(<SendLinkedScreen model={linkedSameOs} on={on} />);
    expect(screen.queryByText('Moving across systems')).not.toBeInTheDocument();
  });
});

describe('Transfer', () => {
  const on = () => ({
    onCancel: vi.fn(),
    onPause: vi.fn(),
    onResume: vi.fn(),
    onRetry: vi.fn(),
    onSeeWhat: vi.fn(),
    onHome: vi.fn(),
  });

  it('runs the bar between the sending and the receiving computer', () => {
    renderScreen(<RecvProgressScreen model={progressAt('moving')} on={on()} />);
    const bar = screen.getByRole('progressbar', { name: 'Overall progress' });
    const wire = bar.closest<HTMLElement>('.s-live__wire');
    expect(wire).not.toBeNull();
    if (wire === null) return;
    const from = within(wire).getByText('ezio-desktop');
    const to = within(wire).getByText('ezio-thinkpad-new');
    expect(from.compareDocumentPosition(bar)).toBe(Node.DOCUMENT_POSITION_FOLLOWING);
    expect(to.compareDocumentPosition(bar)).toBe(Node.DOCUMENT_POSITION_PRECEDING);
  });

  it('says a lost link resumes on its own, with no manual Resume to press', () => {
    renderScreen(<RecvProgressScreen model={progressAt('reconnecting')} on={on()} />);
    expect(screen.getByText("Will resume on its own when it's back")).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Resume' })).not.toBeInTheDocument();
  });

  it('offers Resume as the primary action once the user paused', async () => {
    const intents = on();
    renderScreen(<RecvProgressScreen model={progressAt('held')} on={intents} />);
    await userEvent.click(screen.getByRole('button', { name: 'Resume' }));
    expect(intents.onResume).toHaveBeenCalledOnce();
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

  it('keeps a cross-system override locked until the user says they understand', async () => {
    const on = { onKeepSafe: vi.fn(), onOverride: vi.fn() };
    renderScreen(<OverrideDialog model={OVERRIDES.cross_os} on={on} />);
    const go = screen.getByRole('button', { name: 'Bring anyway' });
    expect(go).toBeDisabled();
    expect(screen.getByText(/to the same place as on Linux/)).toBeInTheDocument();
    await userEvent.click(screen.getByText('I understand apps on macOS may not use it'));
    expect(screen.getByRole('checkbox', { name: /I understand/ })).toBeChecked();
    await userEvent.click(go);
    expect(on.onOverride).toHaveBeenCalledOnce();
  });
});
