import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { i18n } from '@/i18n/i18n';
import { HistoryScreen } from '@/screens/history/HistoryScreen';
import { historyTimeline, historyUndone } from '@/screens/history/fixtures';
import type { HistoryEntry } from '@/screens/history/model';
import { groupMoves, moveTag } from '@/screens/history/moveWords';

const historyOn = () => ({
  onSelect: vi.fn(),
  onUndo: vi.fn(),
  onResume: vi.fn(),
  onForget: vi.fn(),
  onReceive: vi.fn(),
});

const [received] = historyTimeline.entries;
if (received === undefined) throw new Error('fixture');
const entry = (patch: Partial<HistoryEntry>): HistoryEntry => ({ ...received, ...patch });

describe('history words', () => {
  const t = i18n.t.bind(i18n);

  it('tags a move by how it ended: undone, failed, interrupted, then how it finished', () => {
    expect(moveTag(entry({ kind: 'undone', state: 'rolled_back' }), t)?.label).toBe('Undone');
    expect(moveTag(entry({ state: 'failed' }), t)?.tone).toBe('danger');
    expect(moveTag(entry({ state: 'transferring', resumable: true }), t)?.label).toBe(
      'Interrupted',
    );
    expect(moveTag(entry({ state: 'selected' }), t)).toBeNull();
    expect(moveTag(entry({ kind: 'saved' }), t)?.label).toBe('Saved');
    expect(moveTag(entry({}), t)).toMatchObject({ tone: 'success', label: 'Arrived' });
  });

  it('groups consecutive moves that share a label, in order', () => {
    expect(groupMoves([1, 2, 5, 6, 1], (n) => (n < 3 ? 'low' : 'high'))).toEqual([
      { label: 'low', items: [1, 2] },
      { label: 'high', items: [5, 6] },
      { label: 'low', items: [1] },
    ]);
  });
});

describe('History', () => {
  it('opens the selected move beside the list, with Undo and Forget', async () => {
    const user = userEvent.setup();
    const on = historyOn();
    renderScreen(<HistoryScreen model={historyTimeline} on={on} />);
    const current = screen.getByRole('button', { name: /Received from ezio-desktop/ });
    expect(current).toHaveAttribute('aria-current', 'true');
    expect(screen.getByRole('region', { name: 'Today' })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'October 2026' })).toBeInTheDocument();

    const detail = screen.getByRole('complementary', { name: 'This move' });
    expect(within(detail).getByText('Undo point kept')).toBeInTheDocument();
    expect(within(detail).getByText('Removes').nextSibling).toHaveTextContent('12 new files');
    expect(within(detail).getByText('Puts back').nextSibling).toHaveTextContent('1 of your files');
    expect(within(detail).getByText('Leaves alone').nextSibling).toHaveTextContent(
      'Everything else',
    );
    await user.click(within(detail).getByRole('button', { name: 'Undo this move…' }));
    expect(on.onUndo).toHaveBeenCalledOnce();
    await user.click(within(detail).getByRole('button', { name: 'Forget this move…' }));
    expect(on.onForget).toHaveBeenCalledOnce();

    await user.click(screen.getByRole('button', { name: /Saved to a file/ }));
    expect(on.onSelect).toHaveBeenCalledWith('s-1102');
  });

  it('shows an undone move rewound, newest step first, with nothing left to undo', () => {
    renderScreen(<HistoryScreen model={historyUndone} on={historyOn()} />);
    const detail = screen.getByRole('complementary', { name: 'This move' });
    expect(within(detail).getByText('Back the way it was')).toBeInTheDocument();
    const steps = within(detail).getAllByRole('listitem');
    expect(steps.map((li) => li.querySelector('b')?.textContent)).toEqual(['Undone', 'Started']);
    expect(within(detail).queryByRole('button', { name: /Undo/ })).not.toBeInTheDocument();
  });
});
