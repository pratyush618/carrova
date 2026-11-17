import { describe, expect, it } from 'vitest';
import type { CallRow } from '@/bindings/CallRow';
import { appendPage } from '@/flows/calls/useCallLog';

const row = (id: number): CallRow => ({
  id,
  caller: { number: '+1', name: null },
  when_ms: id,
  duration_s: 1,
  kind: 'incoming',
});

describe('appendPage', () => {
  it('joins a day split across two pages', () => {
    const first = { groups: [{ days_ago: 0, rows: [row(5)] }], next: { when_ms: 5, id: 5 } };
    const older = {
      groups: [
        { days_ago: 0, rows: [row(4)] },
        { days_ago: 2, rows: [row(3)] },
      ],
      next: null,
    };
    expect(appendPage(first, older)).toEqual({
      groups: [
        { days_ago: 0, rows: [row(5), row(4)] },
        { days_ago: 2, rows: [row(3)] },
      ],
      next: null,
    });
  });
});
