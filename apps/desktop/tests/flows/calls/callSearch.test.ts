import { describe, expect, it } from 'vitest';
import { dialable, matchesCaller } from '@/flows/calls/callSearch';

const hanu = { name: 'Hanuman Singh', number: '+91 98765-43210' };
const bare = { name: null, number: '9876543210' };

describe('matchesCaller', () => {
  it('matches a name in any case', () => {
    expect(matchesCaller(hanu, 'hanu')).toBe(true);
    expect(matchesCaller(hanu, 'SINGH')).toBe(true);
    expect(matchesCaller(hanu, 'ravi')).toBe(false);
    expect(matchesCaller(bare, 'hanu')).toBe(false);
  });

  it('matches part of a number, ignoring spaces, dashes and +', () => {
    expect(matchesCaller(hanu, '9876')).toBe(true);
    expect(matchesCaller(hanu, '98765 43')).toBe(true);
    expect(matchesCaller(hanu, '5-432')).toBe(true);
    expect(matchesCaller(hanu, '1234')).toBe(false);
  });

  it('matches across a country code either side', () => {
    expect(matchesCaller(bare, '+91 98765')).toBe(true);
    expect(matchesCaller(hanu, '98765 43210')).toBe(true);
  });

  it('matches everything when empty', () => {
    expect(matchesCaller(bare, '  ')).toBe(true);
  });
});

describe('dialable', () => {
  it('keeps only what a dial takes', () => {
    expect(dialable('+91 (98765) 43-210')).toBe('+919876543210');
    expect(dialable('hanu')).toBe('');
  });
});
