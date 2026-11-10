import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { scan } from '@/ipc/fixtures/scan';
import { selectEdits } from '@/ipc/fixtures/select-edits';
import { select } from '@/ipc/fixtures/select';
import { customizeModel, foundApps, rawFolders, sensitiveAsks } from '@/flows/send/sendViews';
import { displayPath } from '@/flows/shared/paths';

const t = i18n.t.bind(i18n);

describe('send views', () => {
  it('summarises detected apps and asks about every sensitive item, starting on leave', () => {
    const apps = foundApps(scan);
    expect(apps.length).toBe(
      scan.report.apps.filter((a) => a.detected && a.items.length > 0).length,
    );
    const asks = sensitiveAsks(scan);
    expect(asks.length).toBeGreaterThan(0);
    expect(asks.every((a) => a.answer === 'leave')).toBe(true);
  });

  it('answers bring once the user confirms', () => {
    const [ask] = sensitiveAsks(scan);
    if (ask === undefined) throw new Error('fixture has no sensitive item');
    // The engine's reply to ticking and confirming `ssh/keys`.
    expect(sensitiveAsks(select).find((a) => a.key === 'ssh/keys')?.answer).toBe('bring');
  });

  it('filters the Customize tree by app or item name and expands matches', () => {
    const all = customizeModel(scan, { filter: '', expanded: [] }, 0);
    expect(all.apps.every((a) => !a.expanded)).toBe(true);
    const [app] = all.apps;
    if (app === undefined) throw new Error('fixture has no apps');
    const filtered = customizeModel(scan, { filter: app.name, expanded: [] }, 0);
    expect(filtered.apps.map((a) => a.id)).toContain(app.id);
    expect(filtered.apps.every((a) => a.expanded)).toBe(true);
  });

  it("groups by the engine's roles: an app row counts only its ordinary items", () => {
    const git = foundApps(selectEdits).find((a) => a.id === 'git');
    expect(git?.included).toBe(false);
    const ssh = foundApps(selectEdits).find((a) => a.id === 'ssh');
    const sshRows = selectEdits.view.rows.filter((r) => r.key.startsWith('ssh/'));
    const ordinary = sshRows.filter((r) => r.roles.ordinary).length;
    expect(ssh?.what.split(', ').length).toBe(ordinary);
  });

  it('shows paths under their real folder, home as ~', () => {
    const dirs = { home: '/home/ana', user_config: '/home/ana/.config' };
    expect(displayPath('user_config:git/config', dirs)).toBe('~/.config/git/config');
    expect(displayPath('cache:x', dirs)).toBe('cache:x');
  });

  it('lists raw folders above the size filter with their file count', () => {
    const folders = rawFolders(scan, 0, t);
    expect(folders.length).toBe(scan.report.unrecognised.length);
    expect(folders.every((f) => f.key.startsWith('raw:') && !f.picked)).toBe(true);
  });
});
