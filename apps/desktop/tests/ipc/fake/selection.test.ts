import { describe, expect, it } from 'vitest';
import { addPicks, applyEdits, viewOf } from '@/ipc/fake/selection';
import { scan } from '@/ipc/fixtures/scan';
import { select } from '@/ipc/fixtures/select';
import { selectEdits } from '@/ipc/fixtures/select-edits';
import { selectEditsInput } from '@/ipc/fixtures/select-edits-input';
import { selectIssues } from '@/ipc/fixtures/select-issues';

/** The fake's reply to `edits` on `base`, as the engine shapes it. */
const fakeSelect = (base: typeof scan.selection, edits: Parameters<typeof applyEdits>[1]) => {
  const selection = applyEdits(base, edits);
  return { selection, view: viewOf(selection, scan.view) };
};

const reply = (r: typeof scan) => ({ selection: r.selection, view: r.view });

describe("the fake's selection rules, held to the engine", () => {
  it('applies app, path, cache and confirmation edits as the engine did', () => {
    expect(fakeSelect(scan.selection, selectEditsInput)).toEqual(reply(selectEdits));
  });

  it('shows an unconfirmed sensitive item as an issue, then clears it on confirmation', () => {
    const tick = { kind: 'item', item: 'ssh/keys', included: true } as const;
    expect(fakeSelect(scan.selection, [tick])).toEqual(reply(selectIssues));
    const confirm = { kind: 'confirm_sensitive', item: 'ssh/keys', confirmed: true } as const;
    expect(fakeSelect(selectIssues.selection, [confirm])).toEqual(reply(select));
  });

  it('rejects an edit naming an unknown item, like the engine', () => {
    expect(() =>
      applyEdits(scan.selection, [{ kind: 'item', item: 'nope/x', included: true }]),
    ).toThrow();
  });
});

describe("the fake's Manual picks, held to the engine's rules", () => {
  const roots = scan.report.platform.roots;
  const none = { items: {}, export_plugins: [], confirmed_plugins: [] };
  const results = (paths: string[], base = none) =>
    addPicks(base, paths, roots).outcomes.map((o) => o.result);

  it('keys a pick by its most specific root', () => {
    expect(results(['/home/ana/.config/starship.toml'])).toEqual([
      { kind: 'added', item: 'raw:user_config/starship.toml', replaced: [] },
    ]);
  });

  it('refuses paths outside the roots, a whole root, a folder holding one and missing paths', () => {
    expect(
      results(['/etc/hosts', '/home/ana', '/home/ana/.local', '/home/ana/missing']).map((r) =>
        r.kind === 'rejected' ? r.reason : r,
      ),
    ).toEqual([
      { kind: 'outside_roots' },
      { kind: 'is_root', root: 'home' },
      { kind: 'contains_root', root: 'user_data' },
      { kind: 'not_found' },
    ]);
  });

  it('absorbs a pick inside a picked folder, replaces picked children, ignores duplicates', () => {
    const site = 'raw:home/projects/site';
    expect(
      results([
        '/home/ana/projects/site',
        '/home/ana/projects/site/index.html',
        '/home/ana/projects',
        '/home/ana/projects',
      ]),
    ).toEqual([
      { kind: 'added', item: site, replaced: [] },
      { kind: 'absorbed', into: site },
      { kind: 'added', item: 'raw:home/projects', replaced: [site] },
      { kind: 'duplicate', item: 'raw:home/projects' },
    ]);
  });

  it('starts a sensitive pick unticked and lists picks apart from rows, as ~/ paths', () => {
    const { selection } = addPicks(
      none,
      ['/home/ana/projects/site', '/home/ana/.ssh/id_ed25519'],
      roots,
    );
    expect(selection.items['raw:home/.ssh/id_ed25519']).toMatchObject({
      included: false,
      sensitive: true,
    });
    const view = viewOf(selection, scan.view, roots);
    expect(view.rows).toEqual([]);
    expect(view.picks.map((p) => [p.display, p.included])).toEqual([
      ['~/.ssh/id_ed25519', false],
      ['~/projects/site', true],
    ]);
    expect(view.issues).toEqual([]);
  });

  it('removes picks only', () => {
    const { selection } = addPicks(none, ['/home/ana/notes.txt'], roots);
    const item = 'raw:home/notes.txt';
    expect(applyEdits(selection, [{ kind: 'remove_pick', item }]).items).toEqual({});
    expect(() =>
      applyEdits(scan.selection, [{ kind: 'remove_pick', item: 'git/config' }]),
    ).toThrow();
  });
});
