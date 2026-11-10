import { describe, expect, it } from 'vitest';
import { flatten } from '@/i18n/catalog';
import { en } from '@/i18n/en';
import { i18n } from '@/i18n/i18n';
import { PSEUDO_EXPANSION, PSEUDO_LOCALE, pseudoLocalize, pseudoMessage } from '@/i18n/pseudo';

const english = flatten(en);
const pseudo = new Map(flatten(pseudoLocalize(en)));
const placeholders = (s: string): string[] =>
  (s.match(/\{\{[^}]+\}\}|<\/?[\w-]+\s*\/?>/g) ?? []).sort();

describe('English catalogue', () => {
  it('has no empty messages', () => {
    expect(english.filter(([, message]) => message.trim() === '')).toEqual([]);
  });

  it('pairs every plural with its other form', () => {
    const keys = new Set(english.map(([key]) => key));
    const ones = [...keys].filter((k) => k.endsWith('_one'));
    expect(ones.filter((k) => !keys.has(k.replace(/_one$/, '_other')))).toEqual([]);
  });
});

describe('pseudo-locale', () => {
  it('covers every English key', () => {
    expect(english.filter(([key]) => !pseudo.has(key)).map(([key]) => key)).toEqual([]);
  });

  it('keeps interpolations and markup intact', () => {
    for (const [key, message] of english) {
      expect(placeholders(pseudo.get(key) ?? ''), key).toEqual(placeholders(message));
    }
  });

  it('accents letters and runs at least 30% longer', () => {
    const out = pseudoMessage('Send');
    expect(out).toMatch(/Š/);
    for (const [key, message] of english) {
      const visible = message.replace(/\{\{[^}]+\}\}|<\/?[\w-]+\s*\/?>/g, '').length;
      expect((pseudo.get(key) ?? '').length, key).toBeGreaterThanOrEqual(
        message.length + Math.ceil(visible * PSEUDO_EXPANSION),
      );
    }
  });

  it('is selectable at runtime and plural-aware', async () => {
    await i18n.changeLanguage(PSEUDO_LOCALE);
    expect(i18n.t('count.apps', { count: 5 })).toMatch(/^⟦5 áƥƥš|^⟦5 /);
    await i18n.changeLanguage('en');
    expect(i18n.t('count.apps', { count: 1 })).toBe('1 app');
    expect(i18n.t('count.apps', { count: 5 })).toBe('5 apps');
  });
});
