import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import { answerEdits, choiceEdits, suggestedPlanOptions } from '@/flows/receive/planOptions';

describe('plan options', () => {
  it('suggests replace with backup and overrides nothing', () => {
    const o = ENGINE_DEFAULTS.plan_options;
    expect(o).toMatchObject({ items: null, default_rule: 'replace', ignore_free_space: false });
  });

  it("starts a new move from the user's Settings defaults", () => {
    const o = suggestedPlanOptions({
      conflict_rule: 'keep-both',
      retention: { keep_for: { days: 30 } },
      include_caches: false,
    });
    expect(o).toMatchObject({ default_rule: 'keep-both', retention: { keep_for: { days: 30 } } });
  });

  it('sends a secret answer for the item and an open-app answer for the app', () => {
    expect(answerEdits('ssh/keys', 'bring')).toEqual([
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: true },
    ]);
    expect(answerEdits('ssh/keys', 'leave')).toEqual([
      { kind: 'answer_sensitive', item: 'ssh/keys', bring: false },
    ]);
    expect(answerEdits('vscode', 'skip')).toEqual([
      { kind: 'skip_app', app: 'vscode', skip: true },
    ]);
    expect(answerEdits('vscode', 'closed')).toEqual([
      { kind: 'skip_app', app: 'vscode', skip: false },
    ]);
  });

  it('sends only the import-wide choices the user changed', () => {
    expect(choiceEdits({ importTrust: true, retention: { keep_for: { days: 7 } } })).toEqual([
      { kind: 'retention', retention: { keep_for: { days: 7 } } },
      { kind: 'import_trust', allow: true },
    ]);
    expect(choiceEdits({ keepBothSuffix: 'old', keepGroupWrite: false })).toEqual([
      { kind: 'keep_both_suffix', suffix: 'old' },
      { kind: 'keep_group_write', keep: false },
    ]);
    expect(choiceEdits({})).toEqual([]);
  });
});
