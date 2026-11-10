import { describe, expect, it } from 'vitest';
import { applyEdits, viewOf } from '@/ipc/fake/selection';
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
