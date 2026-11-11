import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { AppsScreen } from '@/screens/apps/AppsScreen';
import {
  catalogAll,
  catalogSearch,
  catalogSelected,
  catalogSensitive,
} from '@/screens/apps/fixtures';
import type { CatalogIntents } from '@/screens/apps/model';

const catalogOn = (): CatalogIntents => ({
  onFilter: vi.fn(),
  onQuery: vi.fn(),
  onSelect: vi.fn(),
  onShowAll: vi.fn(),
});

describe('Apps', () => {
  it('rows show only exceptions', () => {
    renderScreen(<AppsScreen model={catalogAll} on={catalogOn()} />);
    const git = screen.getByRole('button', { name: /Git/ });
    expect(within(git).queryByText(/here/i)).not.toBeInTheDocument();
    expect(
      within(screen.getByRole('button', { name: /tmux/ })).getByText('Not here'),
    ).toBeVisible();
    expect(
      within(screen.getByRole('button', { name: /Neovim/ })).getByText('Community'),
    ).toBeVisible();
    expect(within(git).queryByText('Verified')).not.toBeInTheDocument();
  });

  it('category count appears once', () => {
    renderScreen(<AppsScreen model={catalogAll} on={catalogOn()} />);
    const heading = screen.getByRole('heading', { name: /Developer tools/ });
    expect(heading).toHaveTextContent('41');
    const more = screen.getAllByRole('button', { name: 'Show all' });
    expect(more.length).toBeGreaterThan(0);
    for (const button of more) expect(button).not.toHaveTextContent(/\d/);
  });

  it('asks to show all of one category', async () => {
    const on = catalogOn();
    renderScreen(<AppsScreen model={catalogAll} on={on} />);
    const [more] = screen.getAllByRole('button', { name: 'Show all' });
    if (more === undefined) throw new Error('no Show all');
    await userEvent.setup().click(more);
    expect(on.onShowAll).toHaveBeenCalledWith('dev-tools');
  });

  it('selecting a row fills the detail pane without an overlay', async () => {
    const on = catalogOn();
    const first = renderScreen(<AppsScreen model={catalogAll} on={on} />);
    const detail = screen.getByRole('complementary', { name: 'App details' });
    expect(within(detail).getByText('Pick an app to see what Carrova moves for it')).toBeVisible();

    await userEvent.setup().click(screen.getByRole('button', { name: /VS Code/ }));
    expect(on.onSelect).toHaveBeenCalledWith('vscode');

    first.unmount();
    renderScreen(<AppsScreen model={catalogSelected} on={on} />);
    const picked = screen.getByRole('complementary', { name: 'App details' });
    expect(within(picked).getByRole('heading', { name: 'VS Code' })).toBeVisible();
    expect(within(picked).getByRole('list', { name: 'What moves' })).toBeVisible();
    expect(screen.getByRole('button', { name: /VS Code/ })).toHaveAttribute('aria-pressed', 'true');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('left-out items are not struck through', () => {
    const { container } = renderScreen(<AppsScreen model={catalogSelected} on={catalogOn()} />);
    const left = screen.getByRole('list', { name: 'Left out' });
    expect(within(left).getAllByRole('listitem').length).toBeGreaterThan(0);
    expect(container.querySelector('s, del, strike, [style*="line-through"]')).toBeNull();
  });

  it('says a sensitive item starts off, and only that one', () => {
    renderScreen(<AppsScreen model={catalogSensitive} on={catalogOn()} />);
    const rows = within(screen.getByRole('list', { name: 'What moves' })).getAllByRole('listitem');
    const cue = 'Starts off, asked first';
    expect(rows.map((row) => within(row).queryByText(cue) !== null)).toEqual([false, true]);
  });

  it('search highlights matches', () => {
    const { container } = renderScreen(<AppsScreen model={catalogSearch} on={catalogOn()} />);
    const hits = [...container.querySelectorAll('mark')].map((m) => m.textContent);
    expect(hits).toEqual(['VS', 'VS']);
  });

  it('green trust box is for verified apps only', () => {
    const trustOf = (tier: 'verified' | 'community' | 'unverified') => {
      if (catalogSelected.selected === null) throw new Error('fixture has no selection');
      const selected = { ...catalogSelected.selected, tier };
      const { container, unmount } = renderScreen(
        <AppsScreen model={{ ...catalogSelected, selected }} on={catalogOn()} />,
      );
      const cls = container.querySelector('.s-detail__trust')?.className;
      unmount();
      return cls;
    };
    expect(trustOf('verified')).toContain('s-detail__trust--verified');
    expect(trustOf('community')).toContain('s-detail__trust--community');
    expect(trustOf('unverified')).toContain('s-detail__trust--unverified');
  });
});
