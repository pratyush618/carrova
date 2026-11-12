import { describe, expect, it } from 'vitest';
import type { Notice } from '@/engine/types';
import { bellContent } from '@/flows/inbox/inboxViews';
import { i18n } from '@/i18n/i18n';

const format = {
  bytes: String,
  number: String,
  time: () => '14:02',
  date: () => '9 Nov',
  day: () => 'Yesterday',
};

const NOW = new Date(2026, 10, 12, 15, 0, 0);
const at = (hours: number): number => Math.floor(NOW.getTime() / 1000) - hours * 3600;

const notice = (patch: Partial<Notice>): Notice => ({
  id: 1,
  kind: 'move_finished',
  at: at(1),
  read: false,
  peer: null,
  summary: 'The move finished.',
  ...patch,
});

describe('bell content', () => {
  it('counts unread notices', () => {
    const content = bellContent(
      [notice({ id: 1 }), notice({ id: 2, read: true }), notice({ id: 3 })],
      NOW,
      i18n.t,
      format,
    );
    expect(content.unread).toBe(2);
  });

  it('groups today first, then earlier, keeping the order it was given', () => {
    const content = bellContent(
      [notice({ id: 1, at: at(1) }), notice({ id: 2, at: at(30) }), notice({ id: 3, at: at(40) })],
      NOW,
      i18n.t,
      format,
    );
    expect(content.groups.map((g) => [g.title, g.rows.map((r) => r.id)])).toEqual([
      ['Today', [1]],
      ['Earlier', [2, 3]],
    ]);
  });

  it('leaves out a group with nothing in it', () => {
    expect(bellContent([], NOW, i18n.t, format).groups).toEqual([]);
  });

  it('offers Review only on a request, and names the device beside its words', () => {
    const [group] = bellContent(
      [
        notice({
          id: 1,
          kind: 'send_request',
          peer: "Ana's laptop",
          summary: 'wants to send you files.',
        }),
        notice({ id: 2, kind: 'move_failed', summary: 'The move failed.' }),
      ],
      NOW,
      i18n.t,
      format,
    ).groups;
    expect(group?.rows.map((r) => [r.review, r.peer, r.summary])).toEqual([
      [true, "Ana's laptop", 'wants to send you files.'],
      [false, null, 'The move failed.'],
    ]);
  });

  it('shows the time today and the day before', () => {
    const [today, earlier] = bellContent(
      [notice({ id: 1, at: at(1) }), notice({ id: 2, at: at(30) })],
      NOW,
      i18n.t,
      format,
    ).groups;
    expect(today?.rows[0]?.when).toBe('14:02');
    expect(earlier?.rows[0]?.when).toBe('Yesterday, 14:02');
  });
});
