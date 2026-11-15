import { describe, expect, it } from 'vitest';
import type { KnownDeviceView, LinkedDeviceView, NearbyDeviceView } from '@/engine/types';
import {
  knownDeviceLine,
  nearbyEntries,
  visibilityModel,
  wakeFailure,
} from '@/flows/nearby/nearbyViews';

const record = (id: string, name: string, hint = false): KnownDeviceView => ({
  id,
  fingerprint: `${id.toUpperCase()}00·0000`,
  name,
  os: 'linux',
  first_seen: 1_000,
  last_move: 2_000,
  moves: 4,
  has_wake_hint: hint,
});

const answer = (over: Partial<NearbyDeviceView>): NearbyDeviceView => ({
  name: 'new-mbp',
  os: 'mac_os',
  known: false,
  id: null,
  fingerprint: null,
  answered_ms: 50,
  ring: 'inner',
  offers_code: true,
  ...over,
});

describe('nearby views', () => {
  it('keys new devices that share a name apart', () => {
    const { here } = nearbyEntries([answer({}), answer({ answered_ms: 90 }), answer({})], []);
    expect(here.map((d) => d.key)).toEqual(['new:new-mbp', 'new:new-mbp:2', 'new:new-mbp:3']);
  });

  it('keys a new device apart even when another one is named like a suffixed key', () => {
    const names = ['Laptop', 'Laptop', 'Laptop:2'];
    const { here } = nearbyEntries(
      names.map((name) => answer({ name })),
      [],
    );
    expect(new Set(here.map((d) => d.key)).size).toBe(names.length);
  });

  it('lists who answered in answer order, then remembered devices that did not', () => {
    const known = [record('aa', 'Ana'), record('bb', 'Old ThinkPad', true)];
    const found = [
      answer({ name: 'Ana', known: true, id: 'aa', fingerprint: 'AA00·0000', offers_code: false }),
      answer({}),
    ];
    const { here, away } = nearbyEntries(found, known);
    expect(here.map((d) => [d.name, d.kind])).toEqual([
      ['Ana', 'known'],
      ['new-mbp', 'new'],
    ]);
    expect(here[0]).toMatchObject({ id: 'aa', moves: 4, lastMove: new Date(2_000_000) });
    expect(here[1]).toMatchObject({ id: null, key: 'new:new-mbp', moves: 0, lastMove: null });
    expect(away).toEqual([
      expect.objectContaining({ id: 'bb', kind: 'away', ring: 'outer', canWake: true }),
    ]);
  });

  it("keeps the engine's ring instead of recomputing it", () => {
    const { here } = nearbyEntries([answer({ answered_ms: 5_000, ring: 'inner' })], []);
    expect(here[0]?.ring).toBe('inner');
  });

  it('dates the visibility deadline and a known device line', () => {
    expect(visibilityModel({ choice: 'everyone', until: 60 })).toEqual({
      choice: 'everyone',
      until: new Date(60_000),
    });
    expect(visibilityModel({ choice: 'known', until: null }).until).toBeNull();
    expect(knownDeviceLine({ ...record('aa', 'Ana'), last_move: null }).lastMove).toBeNull();
  });

  it('tells a missing wake hint from any other failed wake', () => {
    expect(wakeFailure({ code: 'known_device', message: 'no hint', hint: null })).toBe('no_hint');
    expect(wakeFailure({ code: 'network', message: 'offline', hint: null })).toBe('failed');
    expect(wakeFailure(new Error('boom'))).toBe('failed');
  });

  it('marks linked devices here, adds linked phones that never answer, and leaves away ones away', () => {
    const linked = (
      id: string,
      name: string,
      state: LinkedDeviceView['state'],
    ): LinkedDeviceView => ({
      id,
      name,
      os: id === 'p8' ? 'android' : 'linux',
      fingerprint: 'AAAA·BBBB',
      state,
      features: [],
    });
    const { here, away } = nearbyEntries(
      [answer({ known: true, id: 'a1', name: "Ana's laptop", fingerprint: 'A100·0000' })],
      [record('a1', "Ana's laptop"), record('o3', 'Old ThinkPad')],
      [
        linked('a1', "Ana's laptop", 'linked'),
        linked('p8', 'Pixel 8', 'linked'),
        linked('o3', 'Old ThinkPad', 'away'),
      ],
    );
    expect(here.map((d) => [d.key, d.kind])).toEqual([
      ['a1', 'linked'],
      ['p8', 'linked'],
    ]);
    expect(away.map((d) => [d.key, d.kind])).toEqual([['o3', 'away']]);
  });
});
