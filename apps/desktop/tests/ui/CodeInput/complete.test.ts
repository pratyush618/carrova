import { describe, expect, it } from 'vitest';
import { acceptSuggestion, suggestWord } from '@/ui/CodeInput/complete';

const WORDS = ['zebra', 'zesty', 'lemon', 'lemur', 'level'];

describe('pairing-word completion', () => {
  it('waits for three letters before suggesting', () => {
    expect(suggestWord('7-ze', WORDS)).toBeNull();
    expect(suggestWord('7-zeb', WORDS)).toBe('zebra');
  });

  it('suggests nothing once the word is complete', () => {
    expect(suggestWord('7-zebra', WORDS)).toBeNull();
  });

  it('accepts a word and adds the next dash until the code is complete', () => {
    expect(acceptSuggestion('7-zeb', WORDS)).toBe('7-zebra-');
    expect(acceptSuggestion('7-zebra-lem', WORDS)).toBe('7-zebra-lemon');
  });

  it('is case-insensitive on the typed prefix', () => {
    expect(suggestWord('7-ZEB', WORDS)).toBe('zebra');
  });
});
