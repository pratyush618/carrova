import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { SendFoundScreen } from '@/screens/send/SendFoundScreen';
import { foundReady, HERE, SECRETS } from '@/screens/send/fixtures';
import type { FoundModel } from '@/screens/send/model';

const foundOn = () => ({
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
});

const render = (model: FoundModel, on = foundOn()) => {
  renderScreen(<SendFoundScreen model={model} purpose="send" here={HERE} on={on} />);
  return on;
};

describe("Here's what we found: filter and presets", () => {
  it('filters as you type and starts from a preset through the selection intents', async () => {
    const user = userEvent.setup();
    const on = render(foundReady);
    await user.type(screen.getByRole('searchbox', { name: 'Filter apps' }), 'g');
    expect(on.onFilter).toHaveBeenCalledWith('g');
    const presets = screen.getByRole('radiogroup', { name: 'Start from' });
    expect(within(presets).getByRole('radio', { name: 'Suggested' })).toBeChecked();
    await user.click(within(presets).getByRole('radio', { name: 'Everything' }));
    expect(on.onPreset).toHaveBeenCalledWith('everything');
  });

  it('says so when the filter matches nothing', () => {
    render({ ...foundReady, groups: [], secrets: [], filter: 'zzz' });
    expect(screen.getByText('No apps match “zzz”')).toBeInTheDocument();
  });
});

describe("Here's what we found: keys and secrets", () => {
  it("shows each key with its app's own glyph", () => {
    render(foundReady);
    const keys = screen.getByRole('list', { name: 'Keys and secrets' });
    for (const secret of SECRETS) {
      expect(within(keys).getByText(secret.app.glyph)).toBeInTheDocument();
    }
  });

  it('the heading box asks for the next key, one at a time, never all at once', async () => {
    const user = userEvent.setup();
    const [first, ...rest] = SECRETS;
    if (first === undefined) throw new Error('fixture');
    const on = render({ ...foundReady, secrets: [{ ...first, brought: true }, ...rest] });
    const all = screen.getByRole('checkbox', { name: 'Bring keys and secrets, each asked first' });
    expect(all).toHaveAttribute('aria-checked', 'mixed');
    await user.click(all);
    expect(on.onConfirmKey.mock.calls).toEqual([[rest[0]?.key]]);
  });

  it('the heading box leaves every key behind once all are brought', async () => {
    const user = userEvent.setup();
    const on = render({ ...foundReady, secrets: SECRETS.map((s) => ({ ...s, brought: true })) });
    await user.click(
      screen.getByRole('checkbox', { name: 'Bring keys and secrets, each asked first' }),
    );
    expect(on.onConfirmKey).not.toHaveBeenCalled();
    expect(on.onLeaveKey.mock.calls).toEqual(SECRETS.map((s) => [s.key]));
  });
});
