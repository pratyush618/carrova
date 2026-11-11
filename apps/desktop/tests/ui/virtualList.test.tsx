import { act, fireEvent, render, screen } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { VirtualList } from '@/ui';

const VIEWPORT = 400;
const ROW = 40;
const items = Array.from({ length: 5000 }, (_, i) => `Item ${String(i)}`);

const list = (overscan?: number) => (
  <VirtualList
    items={items}
    rowHeight={ROW}
    {...(overscan !== undefined && { overscan })}
    render={(item) => <li key={item}>{item}</li>}
  />
);

describe('VirtualList', () => {
  beforeEach(() => {
    // jsdom lays nothing out: give the scroll container a viewport.
    vi.spyOn(HTMLElement.prototype, 'clientHeight', 'get').mockReturnValue(VIEWPORT);
  });
  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('virtual list renders at most visible+overscan rows for 5000 items', () => {
    render(list());
    const shown = screen.getAllByRole('listitem').length;
    expect(shown).toBeGreaterThan(0);
    expect(shown).toBeLessThanOrEqual(VIEWPORT / ROW + 8 * 2);
    expect(screen.getByText('Item 0')).toBeInTheDocument();
    expect(screen.queryByText('Item 4999')).not.toBeInTheDocument();
  });

  it('follows the scroll position', () => {
    const { container } = render(list(2));
    const scroller = container.firstElementChild;
    if (scroller === null) throw new Error('scroller');
    act(() => {
      fireEvent.scroll(scroller, { target: { scrollTop: ROW * 1000 } });
    });
    expect(screen.getByText('Item 1000')).toBeInTheDocument();
    expect(screen.getByText('Item 998')).toBeInTheDocument();
    expect(screen.queryByText('Item 0')).not.toBeInTheDocument();
    expect(screen.getAllByRole('listitem').length).toBeLessThanOrEqual(VIEWPORT / ROW + 2 * 2);
  });
});
