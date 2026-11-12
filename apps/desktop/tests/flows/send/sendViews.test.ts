import { describe, expect, it } from 'vitest';
import { i18n } from '@/i18n/i18n';
import { scan } from '@/ipc/fixtures/scan';
import { selectEdits } from '@/ipc/fixtures/select-edits';
import { select } from '@/ipc/fixtures/select';
import { createFakePort } from '@/ipc/fake';
import {
  activePreset,
  customizeModel,
  filterFound,
  foundApps,
  foundGroups,
  manualFacts,
  moveFacts,
  rawFolders,
  secretItems,
} from '@/flows/send/sendViews';
import { displayPath } from '@/flows/shared/paths';
import { sensitiveWhy } from '@/screens/shared/sensitiveWhy';

const t = i18n.t.bind(i18n);

describe('send views', () => {
  it('groups apps with ordinary items by category, in scan order', () => {
    const groups = foundGroups(scan);
    expect(groups.map((g) => g.category)).toEqual(['dev-tools', 'editors', 'security', 'shells']);
    expect(groups.find((g) => g.category === 'shells')?.apps.map((a) => a.id)).toEqual([
      'bash',
      'starship',
    ]);
  });

  it('lists every movable secret with its key files, starting off', () => {
    const secrets = secretItems(scan);
    expect(secrets.map((s) => s.key)).toContain('ssh/keys');
    expect(secrets.every((s) => !s.brought)).toBe(true);
    const keys = secrets.find((s) => s.key === 'ssh/keys');
    expect(keys?.paths.length).toBeGreaterThan(0);
    expect(keys?.paths.every((p) => !p.includes(':'))).toBe(true);
  });

  it('says why each secret matters in plain words, never the recipe flag', () => {
    const why = Object.fromEntries(secretItems(scan).map((s) => [s.key, s.why]));
    expect(why).toMatchObject({
      'ssh/keys': 'login',
      'ssh/authorized-keys': 'authorized',
      'git/credentials': 'token',
    });
    expect(sensitiveWhy('someapp/secret')).toBe('secret');
  });

  it('lists the key files found in a secret with their fingerprints', () => {
    const key = {
      path: 'home:.ssh/id_ed25519',
      kind: 'ed25519',
      fingerprint: 'SHA256:qX3fAbc',
      comment: 'ana@laptop',
    } as const;
    const withKeys = {
      ...scan,
      report: {
        ...scan.report,
        apps: scan.report.apps.map((a) => ({
          ...a,
          items: a.items.map((i) => (`${a.id}/${i.id}` === 'ssh/keys' ? { ...i, keys: [key] } : i)),
        })),
      },
    };
    const ssh = secretItems(withKeys).find((s) => s.key === 'ssh/keys');
    expect(ssh?.keys).toEqual([
      { ...key, path: displayPath(key.path, scan.report.platform.roots) },
    ]);
    expect(ssh?.keys[0]?.path).not.toContain(':');
  });

  it('counts a secret as brought once the user confirms it', () => {
    // The engine's reply to ticking and confirming `ssh/keys`.
    expect(secretItems(select).find((s) => s.key === 'ssh/keys')?.brought).toBe(true);
    expect(moveFacts(select).keys).toBe(1);
  });

  it('states the move in facts: apps ticked of found, bytes ticked, no keys, no caches', () => {
    const facts = moveFacts(scan);
    const apps = foundGroups(scan).flatMap((g) => g.apps);
    expect(facts).toMatchObject({
      apps: apps.filter((a) => a.included).length,
      totalApps: apps.length,
      keys: 0,
      caches: false,
    });
    const ticked = Object.entries(scan.selection.items).filter(([, c]) => c.included);
    expect(facts.bytes).toBeGreaterThan(0);
    expect(ticked.length).toBeGreaterThan(0);
  });

  it('counts brought sensitive Manual picks as secrets, not keys', async () => {
    const port = createFakePort({ stepMs: 0 });
    const { session, selection } = await port.startManual();
    const env = '/home/ana/work/api/.env';
    const picked = await port.pick(session, selection, [env]);
    const item = 'raw:home/work/api/.env';
    const brought = await port.select(session, picked.scan.selection, [
      { kind: 'item', item, included: true },
      { kind: 'confirm_sensitive', item, confirmed: true },
    ]);
    expect(manualFacts(brought)).toMatchObject({ picks: 1, keys: 0, secrets: 1 });
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

  it('marks an app changed when one of its items differs from the suggestion', () => {
    // Neovim suggests its config but not its plugins; ticking the plugins changes it.
    const plugins = scan.selection.items['neovim/plugins'];
    if (plugins === undefined) throw new Error('fixture lost neovim/plugins');
    const edited = {
      ...scan,
      selection: {
        ...scan.selection,
        items: { ...scan.selection.items, 'neovim/plugins': { ...plugins, included: true } },
      },
    };
    const neovim = (result: typeof scan) => foundApps(result).find((a) => a.id === 'neovim');
    expect(neovim(scan)?.changed).toBe(false);
    expect(neovim(edited)?.changed).toBe(true);
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

  it('filters found apps and secrets by app or item name, dropping emptied groups', () => {
    const all = { groups: foundGroups(scan), secrets: secretItems(scan) };
    expect(filterFound(all.groups, all.secrets, '  ', 'all')).toEqual({ ...all, narrowed: false });
    const ssh = filterFound(all.groups, all.secrets, 'SSH', 'all');
    expect(ssh.groups.map((g) => g.category)).toEqual(['security']);
    expect(ssh.groups.flatMap((g) => g.apps.map((a) => a.id))).toEqual(['ssh']);
    expect(ssh.secrets.every((s) => s.app.id === 'ssh')).toBe(true);
    expect(ssh.secrets.length).toBeGreaterThan(0);
    expect(filterFound(all.groups, all.secrets, 'no such app', 'all')).toEqual({
      groups: [],
      secrets: [],
      narrowed: true,
    });
  });

  it('changed-by-you filter lists only rows differing from the suggestion', () => {
    const app = (id: string, included: boolean, changed: boolean, bytes = 1) => ({
      id,
      glyph: id,
      name: id,
      what: '',
      files: 1,
      bytes,
      included,
      changed,
    });
    const groups = [
      {
        category: 'a',
        collapsed: true,
        apps: [app('kept', true, false), app('added', true, true)],
      },
      {
        category: 'b',
        collapsed: true,
        apps: [app('dropped', false, true, 9), app('left', false, false, 5)],
      },
    ];
    const ids = (view: 'all' | 'ticked' | 'changed' | 'largest') =>
      filterFound(groups, [], '', view).groups.flatMap((g) => g.apps.map((a) => a.id));
    expect(ids('changed')).toEqual(['added', 'dropped']);
    expect(ids('ticked')).toEqual(['kept', 'added']);
    expect(ids('largest')).toEqual(['dropped', 'left', 'kept', 'added']);
    expect(filterFound(groups, [], '', 'changed').groups.every((g) => !g.collapsed)).toBe(true);
  });

  it('collapses every group of a big machine, until a filter or search narrows it', () => {
    const apps = Array.from({ length: 8 }, () => scan.report.apps).flat();
    const big = foundGroups({ ...scan, report: { ...scan.report, apps } });
    expect(big.flatMap((g) => g.apps).length).toBeGreaterThan(30);
    expect(big.every((g) => g.collapsed)).toBe(true);
    expect(filterFound(big, [], '', 'all').groups.every((g) => g.collapsed)).toBe(true);
    expect(filterFound(big, [], 'a', 'all').groups.every((g) => !g.collapsed)).toBe(true);
    expect(filterFound(big, [], '', 'ticked').groups.every((g) => !g.collapsed)).toBe(true);
    expect(foundGroups(scan).every((g) => !g.collapsed)).toBe(true);
  });

  it('names the preset the selection matches, ignoring keys (they are asked one by one)', () => {
    const { items } = select.selection;
    const every = (pick: (c: (typeof items)[string]) => boolean) => ({
      ...select.selection,
      items: Object.fromEntries(
        Object.entries(items).map(([k, c]) => [k, { ...c, included: pick(c) }]),
      ),
    });
    // `select` brought `ssh/keys`; the apps are still as suggested.
    expect(activePreset(select.selection)).toBe('suggested');
    expect(activePreset(every((c) => !c.sensitive && c.unmovable === null))).toBe('everything');
    expect(activePreset(every((c) => c.sensitive))).toBe('none');
    expect(activePreset(every((c) => c.suggested || c.paths.includes('user_data:nvim')))).toBe(
      null,
    );
  });
});
