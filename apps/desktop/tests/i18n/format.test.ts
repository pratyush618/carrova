import { describe, expect, it } from 'vitest';
import { formatBytes, formatDay, isNearDay, minutesLeft } from '@/i18n/format';

describe('formatBytes', () => {
  it('uses decimal units with one decimal below ten', () => {
    expect(formatBytes(1_200_000_000, 'en')).toBe('1.2 GB');
    expect(formatBytes(612_000_000, 'en')).toBe('612 MB');
    expect(formatBytes(4_000, 'en')).toBe('4 kB');
    expect(formatBytes(0, 'en')).toBe('0 byte');
  });

  it('follows the locale', () => {
    expect(formatBytes(1_200_000_000, 'de')).toMatch(/^1,2\sGB$/);
  });
});

describe('formatDay', () => {
  const now = new Date(2026, 10, 8, 10, 30);

  it('says yesterday, capitalised only at a sentence start', () => {
    const y = new Date(2026, 10, 7, 14, 2);
    expect(formatDay(y, now, 'en')).toBe('yesterday');
    expect(formatDay(y, now, 'en', true)).toBe('Yesterday');
    expect(isNearDay(y, now)).toBe(true);
  });

  it('falls back to a short date further away', () => {
    expect(formatDay(new Date(2026, 10, 2), now, 'en-GB')).toBe('2 Nov');
  });
});

describe('minutesLeft', () => {
  it('rounds to whole minutes, never below one', () => {
    expect(minutesLeft(20)).toBe(1);
    expect(minutesLeft(130)).toBe(2);
  });
});
