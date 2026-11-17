import { describe, expect, it, vi } from 'vitest';
import type { Cursor } from '@/bindings/Cursor';
import { loadPages } from '@/flows/phone/pages';

interface Page {
  readonly rows: readonly number[];
  readonly next: Cursor | null;
}

/** Rows 9..0 in pages of 4, newest first. */
const load = vi.fn((before: Cursor | null): Promise<Page> => {
  const from = before === null ? 9 : before.id - 1;
  const rows = [from, from - 1, from - 2, from - 3].filter((n) => n >= 0);
  const last = rows.at(-1) ?? 0;
  return Promise.resolve({ rows, next: last > 0 ? { when_ms: last, id: last } : null });
});
const join = (newer: Page, older: Page): Page => ({
  rows: [...newer.rows, ...older.rows],
  next: older.next,
});

describe('loadPages', () => {
  it('chains each page from where the last one ended', async () => {
    load.mockClear();
    expect(await loadPages(load, 2, join)).toEqual({
      rows: [9, 8, 7, 6, 5, 4, 3, 2],
      next: { when_ms: 2, id: 2 },
    });
    expect(load.mock.calls.map(([before]) => before?.id ?? null)).toEqual([null, 6]);
  });

  it('stops at the last page', async () => {
    load.mockClear();
    expect((await loadPages(load, 9, join)).rows).toHaveLength(10);
    expect(load).toHaveBeenCalledTimes(3);
  });
});
