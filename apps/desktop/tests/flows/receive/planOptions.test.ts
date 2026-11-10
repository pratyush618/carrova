import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import type { OfferedItem } from '@/engine/types';
import {
  allowCrossOs,
  allowPlugin,
  answerSensitive,
  applyOverride,
  setItemRule,
  setPathRule,
  setChoices,
  setRemap,
  skipApp,
} from '@/flows/receive/planOptions';

const offered: OfferedItem[] = [
  { key: 'ssh/config', label: 'Config', picked: true, sensitive: null },
  {
    key: 'ssh/keys',
    label: 'Keys',
    picked: false,
    sensitive: { reason: 'Private keys', confirmed: false },
  },
  { key: 'vscode/settings', label: 'Settings', picked: true, sensitive: null },
];

describe('plan options', () => {
  it('suggests replace with backup and overrides nothing', () => {
    const o = ENGINE_DEFAULTS.plan_options;
    expect(o).toMatchObject({ items: null, default_rule: 'replace', ignore_free_space: false });
  });

  it('confirms a sensitive item when the user brings it and forgets it when they leave it', () => {
    const bring = answerSensitive(ENGINE_DEFAULTS.plan_options, offered, 'ssh/keys', true);
    expect(bring.confirmed_sensitive).toEqual(['ssh/keys']);
    expect(bring.items).toBeNull();
    const leave = answerSensitive(bring, offered, 'ssh/keys', false);
    expect(leave.confirmed_sensitive).toEqual([]);
    expect(leave.items).toEqual(['ssh/config', 'vscode/settings']);
  });

  it('allows a same-system item once, however often the user asks', () => {
    const once = allowCrossOs(ENGINE_DEFAULTS.plan_options, 'editor/state');
    expect(allowCrossOs(once, 'editor/state').allow_cross_os).toEqual(['editor/state']);
  });

  it('picks a same-system item again when the user overrides after leaving it', () => {
    const left = { ...ENGINE_DEFAULTS.plan_options, items: ['ssh/config'] };
    expect(allowCrossOs(left, 'editor/state').items).toEqual(['ssh/config', 'editor/state']);
  });

  it('allows a plugin once, however often the user asks', () => {
    const once = allowPlugin(ENGINE_DEFAULTS.plan_options, 'path-rewrite');
    expect(allowPlugin(once, 'path-rewrite').confirmed_plugins).toEqual(['path-rewrite']);
  });

  it('skips every item of an open app and brings them back', () => {
    const skipped = skipApp(ENGINE_DEFAULTS.plan_options, offered, 'vscode', true);
    expect(skipped.items).toEqual(['ssh/config']);
    expect(skipApp(skipped, offered, 'vscode', false).items).toEqual([
      'ssh/config',
      'vscode/settings',
    ]);
  });

  it('sets rules per item and per file, a file rule cleared back to the item rule', () => {
    let o = setItemRule(ENGINE_DEFAULTS.plan_options, 'ssh/config', 'keep-both');
    o = setPathRule(o, 'home:.ssh/config', 'skip');
    expect(o.item_rules).toEqual({ 'ssh/config': 'keep-both' });
    expect(o.path_rules).toEqual({ 'home:.ssh/config': 'skip' });
    expect(setPathRule(o, 'home:.ssh/config', null).path_rules).toEqual({});
  });

  it('remaps an item and removes the remap again', () => {
    const remap = { root: 'documents' as const, rel_prefix: 'old-setup' };
    const o = setRemap(ENGINE_DEFAULTS.plan_options, 'ssh/config', remap);
    expect(o.remaps).toEqual({ 'ssh/config': remap });
    expect(setRemap(o, 'ssh/config', null).remaps).toEqual({});
  });

  it('turns on exactly the override the user confirmed', () => {
    expect(applyOverride(ENGINE_DEFAULTS.plan_options, 'space')).toMatchObject({
      ignore_free_space: true,
      ignore_requires_closed: false,
    });
  });

  it('applies import-wide choices and keeps the suggested suffix when cleared', () => {
    const o = setChoices(ENGINE_DEFAULTS.plan_options, {
      importTrust: true,
      retention: 'discard_on_success',
    });
    expect(o).toMatchObject({ import_trust: true, retention: 'discard_on_success' });
    expect(setChoices(o, { keepBothSuffix: '  ' }).keep_both_suffix).toBe('carrova-{date}');
    expect(setChoices(o, { keepBothSuffix: 'old' }).keep_both_suffix).toBe('old');
  });
});
