import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { SendFoundScreen } from '@/screens/send/SendFoundScreen';
import { foundReady, HERE, SECRETS } from '@/screens/send/fixtures';
import type { FoundApp, FoundGroup, FoundModel } from '@/screens/send/model';

const foundOn = () => ({
  onToggleApp: vi.fn(),
  onConfirmKey: vi.fn(),
  onLeaveKey: vi.fn(),
  onReviewPlugin: vi.fn(),
  onFilter: vi.fn(),
  onFoundFilter: vi.fn(),
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

  it('reopens a group you closed when the search changes again', async () => {
    const user = userEvent.setup();
    function Searching() {
      const [filter, setFilter] = useState('');
      const model = { ...foundReady, filter };
      return (
        <SendFoundScreen
          model={model}
          purpose="send"
          here={HERE}
          on={{ ...foundOn(), onFilter: setFilter }}
        />
      );
    }
    renderScreen(<Searching />);
    const search = screen.getByRole('searchbox', { name: 'Filter apps' });
    await user.type(search, 'v');
    await user.click(screen.getByRole('button', { name: /Editors/ }));
    expect(screen.queryByRole('list', { name: 'Editors' })).not.toBeInTheDocument();
    await user.type(search, 's');
    expect(screen.getByRole('list', { name: 'Editors' })).toBeInTheDocument();
  });

  it('reopens a group you closed once a search narrows the list', async () => {
    const user = userEvent.setup();
    function Narrowing() {
      const [filter, setFilter] = useState('');
      const model = { ...foundReady, filter };
      return (
        <SendFoundScreen
          model={model}
          purpose="send"
          here={HERE}
          on={{ ...foundOn(), onFilter: setFilter }}
        />
      );
    }
    renderScreen(<Narrowing />);
    await user.click(screen.getByRole('button', { name: /Editors/ }));
    expect(screen.queryByRole('list', { name: 'Editors' })).not.toBeInTheDocument();
    await user.type(screen.getByRole('searchbox', { name: 'Filter apps' }), 'v');
    expect(screen.getByRole('list', { name: 'Editors' })).toBeInTheDocument();
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

const bulkApp = (n: number, category: string): FoundApp => ({
  id: `${category}-${String(n)}`,
  glyph: 'ap',
  name: `${category} app ${String(n)}`,
  what: 'Settings',
  files: 1,
  bytes: 1_000 + n,
  included: n % 3 === 0,
  changed: false,
});

/** 340 apps over 8 categories (one of 240): every one collapsed, as the engine's view marks them. */
const BULK_CATEGORIES = ['dev-tools', 'editors', 'notes', 'security', 'shells', 'terminals'];
const bulkGroups = (): FoundGroup[] => {
  const sizes = [240, 20, 20, 20, 20, 20];
  return BULK_CATEGORIES.map((category, i) => ({
    category,
    collapsed: true,
    apps: Array.from({ length: sizes[i] ?? 0 }, (_, n) => bulkApp(n, category)),
  }));
};
const bulk: FoundModel = {
  ...foundReady,
  groups: bulkGroups(),
  facts: { ...foundReady.facts, apps: 115, totalApps: 340 },
};

describe("Here's what we found: at scale", () => {
  it('pick with 340 apps renders keys open and other groups collapsed', () => {
    render(bulk);
    expect(
      within(screen.getByRole('list', { name: 'Keys and secrets' })).getAllByRole('listitem'),
    ).toHaveLength(SECRETS.length);
    for (const name of ['Developer tools', 'Editors', 'Terminals']) {
      expect(screen.getByRole('button', { name: new RegExp(name) })).toHaveAttribute(
        'aria-expanded',
        'false',
      );
    }
    expect(screen.queryByRole('list', { name: 'Developer tools' })).not.toBeInTheDocument();
  });

  it('an opened group shows 8 rows, then the rest on request', async () => {
    const user = userEvent.setup();
    render(bulk);
    await user.click(screen.getByRole('button', { name: /Editors/ }));
    expect(
      within(screen.getByRole('list', { name: 'Editors' })).getAllByRole('listitem'),
    ).toHaveLength(8);
    await user.click(screen.getByRole('button', { name: 'Show 12 more in Editors' }));
    expect(
      within(screen.getByRole('list', { name: 'Editors' })).getAllByRole('listitem'),
    ).toHaveLength(20);
    expect(
      screen.queryByRole('button', { name: /Show .* more in Editors/ }),
    ).not.toBeInTheDocument();
  });

  it('a group over 200 rows renders only the rows in view', async () => {
    const user = userEvent.setup();
    vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(400);
    render(bulk);
    await user.click(screen.getByRole('button', { name: /Developer tools/ }));
    await user.click(screen.getByRole('button', { name: 'Show 232 more in Developer tools' }));
    const rows = within(screen.getByRole('list', { name: 'Developer tools' })).getAllByRole(
      'listitem',
    );
    expect(rows.length).toBeGreaterThan(8);
    expect(rows.length).toBeLessThan(60);
    vi.restoreAllMocks();
  });

  it('shows how many are ticked and offers All, Ticked, Changed by you and Largest', async () => {
    const user = userEvent.setup();
    const on = render(bulk);
    expect(screen.getByText(/115 ticked/)).toBeInTheDocument();
    const views = screen.getByRole('radiogroup', { name: 'Show' });
    expect(
      within(views)
        .getAllByRole('radio')
        .map((r) => r.textContent),
    ).toEqual(['All', 'Ticked', 'Changed by you', 'Largest']);
    await user.click(within(views).getByRole('radio', { name: 'Changed by you' }));
    expect(on.onFoundFilter).toHaveBeenCalledWith('changed');
  });
});
