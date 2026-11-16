import { describe, expect, it } from 'vitest';
import { appsMatching, pendingPerm } from '@/mobile/phone/model';

describe('phone feature helpers', () => {
  it('the first permission not granted is the one asked', () => {
    expect(pendingPerm(['calls', 'contacts'], ['ok', 'ask'])).toEqual({
      perm: 'contacts',
      step: 'ask',
    });
    expect(pendingPerm(['calls', 'contacts'], ['ok', 'ok'])).toBeNull();
    expect(pendingPerm([], [])).toBeNull();
  });

  it('apps match by name or package id, any case', () => {
    const apps = [
      { id: 'org.chat', name: 'Chat', on: true, private: false },
      { id: 'com.bank', name: 'Money', on: false, private: true },
    ];
    expect(appsMatching(apps, 'BANK').map((a) => a.id)).toEqual(['com.bank']);
    expect(appsMatching(apps, ' ')).toEqual(apps);
  });
});
