import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { formatBytes } from '@/i18n/format';
import { visibleEntries } from '@/screens/apps/visibleEntries';
import { catalogAll } from '@/screens/apps/fixtures';
import { unanswered } from '@/screens/receive/ReviewDecisions';
import { reviewDecide, reviewReady } from '@/screens/receive/fixtures';
import { appCheck } from '@/screens/send/CustomizeTree';
import { selectionTotals } from '@/screens/send/FoundSelection';
import { customize, foundReady } from '@/screens/send/fixtures';
import { chipText } from '@/screens/shared/chipText';
import { transferAt, transferMoving } from '@/screens/shared/transferFixtures';

const bytes = (n: number) => formatBytes(n, 'en');

describe('selection rules', () => {
  it('marks an app mixed when only some movable items are picked, ignoring unmovable ones', () => {
    const [ssh, git, vscode] = customize.apps;
    expect(ssh && appCheck(ssh)).toBe('mixed');
    expect(git && appCheck(git)).toBe('mixed');
    expect(vscode && appCheck(vscode)).toBe('mixed');
  });

  it('totals only the apps being brought', () => {
    if (foundReady.status !== 'found') throw new Error('fixture');
    const apps = foundReady.apps.map((a) => (a.id === 'vscode' ? { ...a, included: false } : a));
    expect(selectionTotals(apps).files).toBe(24);
  });

  it('counts unanswered decisions', () => {
    expect(unanswered(reviewDecide.decisions)).toBe(3);
    expect(unanswered(reviewReady.decisions)).toBe(0);
  });
});

describe('chip wording', () => {
  const t = i18n.t.bind(i18n);

  it('gives the moving app its byte count and the rest a plain state', () => {
    const texts = transferMoving.apps.map((app) => chipText(app, transferMoving, t, bytes));
    expect(texts).toEqual(['Arrived', 'Arrived', 'Arrived', 'Arrived', '612 MB of 1.2 GB']);
  });

  it('says "In place" while applying and "Paused" while held', () => {
    const applying = transferAt('applying', 'receive');
    expect(applying.apps[0] && chipText(applying.apps[0], applying, t, bytes)).toBe('In place');
    const paused = transferAt('paused', 'receive');
    expect(paused.apps[4] && chipText(paused.apps[4], paused, t, bytes)).toBe('Paused');
  });
});

describe('catalogue filter', () => {
  it('filters by tier and by name', () => {
    expect(visibleEntries(catalogAll.entries, 'community', '').map((e) => e.id)).toEqual([
      'nvim',
      'kitty',
    ]);
    expect(visibleEntries(catalogAll.entries, 'all', 'vs').map((e) => e.id)).toEqual(['vscode']);
  });
});
