import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import { renderScreen } from '../../support/render';
import { ClipHistorySheet } from '@/screens/clipboard/ClipHistorySheet';

describe('Clipboard history', () => {
  it('ClipHistorySheet lists newest first and clears', async () => {
    const user = userEvent.setup();
    const on = { onClear: vi.fn(), onClose: vi.fn() };
    renderScreen(
      <ClipHistorySheet
        items={[
          { kind: 'text', preview: 'Meet at 7, gate B', at: 1_700_000_200 },
          { kind: 'image', preview: '', at: 1_700_000_100 },
          { kind: 'text', preview: 'https://docs.rs/tokio', at: 1_700_000_000 },
        ]}
        on={on}
      />,
    );
    const rows = within(screen.getByRole('list')).getAllByRole('listitem');
    expect(rows.map((r) => r.querySelector('.p-row__title')?.textContent)).toEqual([
      'Meet at 7, gate B',
      'Image',
      'https://docs.rs/tokio',
    ]);
    await user.click(screen.getByRole('button', { name: 'Clear all' }));
    expect(on.onClear).toHaveBeenCalledOnce();
  });

  it('says when nothing has arrived', () => {
    renderScreen(<ClipHistorySheet items={[]} on={{ onClear: vi.fn(), onClose: vi.fn() }} />);
    expect(screen.getByText('Nothing copied from your devices yet')).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: 'Clear all' })).not.toBeInTheDocument();
  });
});
