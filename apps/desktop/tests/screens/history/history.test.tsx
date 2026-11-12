import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
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
  it('history rows carry no tag for a normal finished move', () => {
    expect(moveTag(entry({}))).toBeNull();
    expect(moveTag(entry({ kind: 'sent', undoable: false }))).toBeNull();
    expect(moveTag(entry({ kind: 'saved', undoable: false }))).toBeNull();
    expect(moveTag(entry({ state: 'selected' }))).toBeNull();
  });

  it('a failed move reads as interrupted, and a move without an undo point gets no tag', () => {
    expect(moveTag(entry({ state: 'failed' }))).toBe('interrupted');
    expect(moveTag(entry({ undoable: false }))).toBeNull();
  });

  it('tags only exceptions: undone, interrupted, undo expired', () => {
    expect(moveTag(entry({ kind: 'undone', state: 'rolled_back' }))).toBe('undone');
    expect(moveTag(entry({ state: 'transferring', resumable: true }))).toBe('interrupted');
    expect(moveTag(entry({ undoExpired: true }))).toBe('undo_expired');
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

  it("badges the other computer's OS and dates an expiring undo point", () => {
    const selected = historyTimeline.selected;
    if (selected === null) throw new Error('fixture');
    const model = {
      ...historyTimeline,
      selected: { ...selected, undoUntil: new Date(2026, 10, 22, 12, 0) },
    };
    renderScreen(<HistoryScreen model={model} on={historyOn()} />);
    const current = screen.getByRole('button', { name: /Received from ezio-desktop/ });
    expect(within(current).getByText('Linux')).toBeInTheDocument();
    const detail = screen.getByRole('complementary', { name: 'This move' });
    expect(within(detail).getByText('Undo available until Nov 22')).toBeInTheDocument();
  });

  it('history detail shows undo deadline once', () => {
    const selected = historyTimeline.selected;
    if (selected === null) throw new Error('fixture');
    const model = {
      ...historyTimeline,
      selected: { ...selected, undoUntil: new Date(2026, 10, 22, 12, 0) },
    };
    renderScreen(<HistoryScreen model={model} on={historyOn()} />);
    expect(screen.getAllByText(/Undo available until/)).toHaveLength(1);
  });

  it('rows show exceptions as words and nothing for a normal move', () => {
    renderScreen(<HistoryScreen model={historyTimeline} on={historyOn()} />);
    expect(screen.queryByText('Arrived')).not.toBeInTheDocument();
    expect(screen.getByText('Undone')).toBeInTheDocument();
    expect(screen.getByText('Undo expired')).toBeInTheDocument();
  });

  it('narrows the list from the header: search by name, then All, Sent, Received, Files', async () => {
    const user = userEvent.setup();
    renderScreen(<HistoryScreen model={historyTimeline} on={historyOn()} />);
    await user.type(screen.getByRole('searchbox', { name: 'Search moves' }), 'lab');
    expect(screen.getAllByRole('button', { name: /Undid a move from lab-desktop/ })).toHaveLength(
      1,
    );
    expect(screen.queryByRole('button', { name: /Saved to a file/ })).not.toBeInTheDocument();
    await user.clear(screen.getByRole('searchbox', { name: 'Search moves' }));
    await user.click(screen.getByRole('radio', { name: 'Files' }));
    expect(screen.getByRole('button', { name: /Saved to a file/ })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /Received from/ })).not.toBeInTheDocument();
    await user.click(screen.getByRole('radio', { name: 'Sent' }));
    expect(screen.getByText('No moves match this filter')).toBeInTheDocument();
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
