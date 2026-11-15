import { describe, expect, it } from 'vitest';
import type { OsKind } from '@/bindings/OsKind';
import { i18n } from '@/i18n/i18n';
import { osName } from '@/screens/send/SendRail';

describe('osName', () => {
  it('names every OS family, Android included', () => {
    const kinds: readonly OsKind[] = ['linux', 'mac_os', 'windows', 'android'];
    expect(kinds.map((os) => osName(os, i18n.t))).toEqual(['Linux', 'macOS', 'Windows', 'Android']);
  });
});
