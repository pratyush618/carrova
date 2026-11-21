import { describe, expect, it } from 'vitest';
import { sendTargets } from '@/flows/send/sendTargets';
import type { DeviceEntry } from '@/screens/nearby/model';

const entry = (over: Partial<DeviceEntry> & Pick<DeviceEntry, 'key' | 'kind'>): DeviceEntry => ({
  id: over.key,
  name: over.key,
  os: 'linux',
  ring: 'inner',
  fingerprint: null,
  lastMove: null,
  moves: 0,
  canWake: false,
  ...over,
});

describe('where a move can go', () => {
  it('lists trusted computers here first, most recent move first', () => {
    const { trusted } = sendTargets({
      here: [
        entry({ key: 'old', kind: 'known', lastMove: new Date(1_000) }),
        entry({ key: 'never', kind: 'known' }),
        entry({ key: 'linked', kind: 'linked', lastMove: new Date(9_000) }),
      ],
      away: [],
    });
    expect(trusted.map((d) => d.key)).toEqual(['linked', 'old', 'never']);
  });

  it('greys trusted computers away and asks new ones for a code once', () => {
    const targets = sendTargets({
      here: [entry({ key: 'new-mbp', kind: 'new', id: null, os: null })],
      away: [entry({ key: 'thinkpad', kind: 'away', canWake: true })],
    });
    expect(targets.trusted).toEqual([]);
    expect(targets.away.map((d) => d.key)).toEqual(['thinkpad']);
    expect(targets.fresh.map((d) => d.key)).toEqual(['new-mbp']);
  });

  it('never offers a phone', () => {
    const phone = { os: 'android' as const };
    const targets = sendTargets({
      here: [
        entry({ key: 'p8', kind: 'linked', ...phone }),
        entry({ key: 'p9', kind: 'new', ...phone }),
      ],
      away: [entry({ key: 'p7', kind: 'away', ...phone })],
    });
    expect(targets).toEqual({ trusted: [], away: [], fresh: [] });
  });
});
