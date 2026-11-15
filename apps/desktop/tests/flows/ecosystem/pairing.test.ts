import { describe, expect, it } from 'vitest';
import { addDeviceModel, attemptInvite, settle } from '@/flows/ecosystem/pairing';
import { pixel, pixelInvite } from '@/screens/devices/fixtures';

const T0 = 1_000_000;
const invite = (receivedAt: number) => ({ view: pixelInvite, receivedAt });

describe('the Add a device model', () => {
  it('counts only the invite that arrived for this attempt', () => {
    expect(attemptInvite(invite(T0 - 1), T0)).toBeNull();
    expect(attemptInvite(null, T0)).toBeNull();
    expect(attemptInvite(invite(T0 + 5), T0)).toEqual({
      view: pixelInvite,
      expiresAt: T0 + 5 + pixelInvite.expires_in_s * 1000,
    });
  });

  it('makes a code, waits with it, then says it expired', () => {
    const current = attemptInvite(invite(T0), T0);
    expect(addDeviceModel(null, null, 0).state).toBe('starting');
    expect(addDeviceModel(current, null, 12)).toMatchObject({
      state: 'waiting',
      invite: pixelInvite,
    });
    expect(addDeviceModel(current, null, 0).state).toBe('expired');
  });

  it('keeps "expired" when the engine then gives up, but a link always wins', () => {
    const expired = settle(null, { state: 'expired' });
    expect(settle(expired, { state: 'failed' })).toEqual({ state: 'expired' });
    expect(addDeviceModel(null, settle(expired, { state: 'failed' }), 0).state).toBe('expired');
    const linked = { state: 'linked', device: pixel } as const;
    expect(settle(expired, linked)).toEqual(linked);
    expect(addDeviceModel(attemptInvite(invite(T0), T0), linked, 0)).toEqual(linked);
  });

  it('shows a failure while the code still runs', () => {
    expect(addDeviceModel(attemptInvite(invite(T0), T0), { state: 'failed' }, 30).state).toBe(
      'failed',
    );
  });
});
