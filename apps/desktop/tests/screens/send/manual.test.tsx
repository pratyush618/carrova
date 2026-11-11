import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import type { PickRejection } from '@/bindings/PickRejection';
import { i18n } from '@/i18n/i18n';
import { renderScreen } from '../../support/render';
import { SendFoundScreen } from '@/screens/send/SendFoundScreen';
import { SendRail } from '@/screens/send/SendRail';
import {
  HERE,
  manualEmpty,
  manualMeasuring,
  manualPicks,
  manualRejected,
  manualSensitive,
  MODE_AUTO,
  MODE_MANUAL,
  foundReady,
} from '@/screens/send/fixtures';
import type { FoundModel, ModeSwitchModel } from '@/screens/send/model';

const on = () => ({
  onToggleApp: vi.fn(),
  onConfirmKey: vi.fn(),
  onLeaveKey: vi.fn(),
  onReviewPlugin: vi.fn(),
  onFilter: vi.fn(),
  onPreset: vi.fn(),
  onCustomize: vi.fn(),
  onPickFolders: vi.fn(),
  onSeeKnownApps: vi.fn(),
  onContinue: vi.fn(),
  onBack: vi.fn(),
  onMode: vi.fn(),
  onAddFiles: vi.fn(),
  onAddFolders: vi.fn(),
  onRemovePick: vi.fn(),
});

const render = (model: FoundModel, mode: ModeSwitchModel = MODE_MANUAL) => {
  const intents = on();
  renderScreen(
    <SendFoundScreen model={model} purpose="send" here={HERE} mode={mode} on={intents} />,
  );
  return intents;
};

describe('Send · Manual mode', () => {
  it('starts with an empty list, the two pickers and nothing to send', async () => {
    const user = userEvent.setup();
    const intents = render(manualEmpty);
    expect(screen.getByRole('heading', { level: 1, name: 'Pick files and folders' })).toBeVisible();
    expect(screen.getByText(/Nothing picked yet/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Add files' }));
    await user.click(screen.getByRole('button', { name: 'Add folder' }));
    expect(intents.onAddFiles).toHaveBeenCalledOnce();
    expect(intents.onAddFolders).toHaveBeenCalledOnce();
    expect(screen.getByRole('button', { name: 'Get pairing code' })).toBeDisabled();
  });

  it('lists each pick with its path and size, and removes one', async () => {
    const user = userEvent.setup();
    const intents = render(manualPicks);
    const list = screen.getByRole('list', { name: 'Your picks' });
    expect(within(list).getByText('~/projects/thesis')).toBeInTheDocument();
    expect(within(list).getByText(/2314 files/)).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Remove ~/projects/thesis' }));
    expect(intents.onRemovePick).toHaveBeenCalledWith('raw:home/projects/thesis');
    expect(screen.getByRole('button', { name: 'Get pairing code' })).toBeEnabled();
  });

  it('asks before a sensitive pick is brought', async () => {
    const user = userEvent.setup();
    const intents = render(manualPicks);
    const box = screen.getByRole('checkbox', { name: 'Bring ~/work/api/.env' });
    expect(box).not.toBeChecked();
    await user.click(box);
    expect(intents.onConfirmKey).toHaveBeenCalledWith('raw:home/work/api/.env');
    await user.click(screen.getByRole('button', { name: 'Include ~/work/api/.env…' }));
    expect(intents.onConfirmKey).toHaveBeenCalledTimes(2);
  });

  it('counts brought sensitive picks under their own rail label, not keys', () => {
    renderScreen(<SendRail move={{ here: HERE, facts: manualSensitive.facts }} purpose="send" />);
    expect(screen.getByText('Sensitive files')).toBeInTheDocument();
    expect(screen.getByText('1 brought')).toBeInTheDocument();
    expect(screen.queryByText('Keys and secrets')).not.toBeInTheDocument();
  });

  it('shows a row while the engine measures a pick', () => {
    render(manualMeasuring);
    const list = screen.getByRole('list', { name: 'Your picks' });
    expect(within(list).getByText('~/Music/sessions')).toBeInTheDocument();
    expect(within(list).getByText('Measuring…')).toBeInTheDocument();
  });

  it('says in plain words why a path was refused', () => {
    render(manualRejected);
    const list = screen.getByRole('list', { name: "Can't add" });
    expect(within(list).getByText('/etc/hosts')).toBeInTheDocument();
    expect(within(list).getByText(/outside your home and app folders/)).toBeInTheDocument();
    expect(within(list).getByText(/your whole home folder/)).toBeInTheDocument();
    expect(
      screen.getByText('~/projects/thesis/notes.md is already inside ~/projects/thesis'),
    ).toBeInTheDocument();
  });

  it("the switch shows the other mode's count and flips the mode", async () => {
    const user = userEvent.setup();
    const intents = render(foundReady, MODE_AUTO);
    expect(screen.getByText('Manual · 2')).toBeInTheDocument();
    const toggle = screen.getByRole('switch', { name: /^Manual/ });
    expect(toggle).not.toBeChecked();
    await user.click(toggle);
    expect(intents.onMode).toHaveBeenCalledWith(true);
  });

  it('has words for every reason the engine refuses a pick', () => {
    const kinds = Object.keys({
      not_found: 0,
      outside_roots: 0,
      is_root: 0,
      contains_root: 0,
      linked_parent: 0,
      symlink_outside: 0,
      special: 0,
      unsupported_name: 0,
      unreadable: 0,
    } satisfies Record<PickRejection['kind'], 0>);
    for (const kind of kinds) {
      expect(i18n.exists(`send.manual.rejected.${kind}`), kind).toBe(true);
    }
  });
});
