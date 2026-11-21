import { describe, expect, it } from 'vitest';
import type { ErrorCode } from '@/bindings/ErrorCode';
import { isBridgeError, isBridgeEvent, toBridgeError } from '@/ipc/guards';

describe('the IPC guards', () => {
  it('keep every error code the engine sends, including the newest', () => {
    const codes: ErrorCode[] = [
      'invalid_selection',
      'rolled_back',
      'rollback_failed',
      'request_declined',
      'request_expired',
      'request_refused',
    ];
    for (const code of codes) {
      const error = { code, message: 'm', hint: null };
      expect(isBridgeError(error)).toBe(true);
      expect(toBridgeError(error)).toEqual(error);
    }
  });

  it('accept a send request or a tray request, which name no session, but no other sessionless event', () => {
    expect(isBridgeEvent({ kind: 'send_requested' })).toBe(true);
    expect(isBridgeEvent({ kind: 'show_nearby' })).toBe(true);
    expect(isBridgeEvent({ kind: 'task_ended', error: null })).toBe(false);
  });

  it('accept a notice, which names no session, only with its text', () => {
    const notice = { id: 1, kind: 'move_finished', at: 1, read: false, peer: null, summary: 's' };
    expect(isBridgeEvent({ kind: 'notice', notice })).toBe(true);
    expect(isBridgeEvent({ kind: 'notice' })).toBe(false);
  });

  it('accept a move request, which names no session, only with who asks, counts and expiry', () => {
    const request = {
      id: 1,
      device: 'd2',
      name: 'Desk PC',
      summary: { apps: 1, bytes: 2, has_sensitive: false },
      expires_at: 3,
    };
    expect(isBridgeEvent({ kind: 'move_request_arrived', request })).toBe(true);
    expect(isBridgeEvent({ kind: 'move_request_arrived', request: { ...request, name: 1 } })).toBe(
      false,
    );
    expect(isBridgeEvent({ kind: 'move_request_ended', id: 1, why: 'expired' })).toBe(true);
    expect(isBridgeEvent({ kind: 'move_request_ended', why: 'expired' })).toBe(false);
    expect(
      isBridgeEvent({ kind: 'move_request_arrived', request: { ...request, device: undefined } }),
    ).toBe(false);
    expect(
      isBridgeEvent({
        kind: 'move_request_arrived',
        request: { ...request, summary: { apps: 1 } },
      }),
    ).toBe(false);
    expect(
      isBridgeEvent({
        kind: 'move_request_arrived',
        request: { ...request, summary: { ...request.summary, has_sensitive: 'no' } },
      }),
    ).toBe(false);
    for (const why of ['cancelled', 'expired', 'superseded', 'lost']) {
      expect(isBridgeEvent({ kind: 'move_request_ended', id: 1, why }), why).toBe(true);
    }
    expect(isBridgeEvent({ kind: 'move_request_ended', id: 1, why: 'mystery' })).toBe(false);
  });

  it('turn an unknown code into an internal error', () => {
    expect(toBridgeError({ code: 'mystery', message: 'm', hint: null }).code).toBe('internal');
  });

  it('accept a pairing invite, which names no session, only with its QR code, code and lifetime', () => {
    const code = {
      code: '4-maple-orbit',
      fallback: null,
      renewed: false,
      expired: false,
      expires_in_secs: 180,
    };
    const invite = { uri: 'carrova://pair?v=1', qr_svg: '<svg/>', code, expires_in_s: 180 };
    expect(isBridgeEvent({ kind: 'pair_invite', invite })).toBe(true);
    expect(isBridgeEvent({ kind: 'pair_invite', invite: { uri: 'x' } })).toBe(false);
    expect(isBridgeEvent({ kind: 'pair_invite', invite: { ...invite, code: {} } })).toBe(false);
    expect(isBridgeEvent({ kind: 'pair_invite', invite: { ...invite, expires_in_s: '180' } })).toBe(
      false,
    );
  });

  it('accept a device found while pairing only with its name, OS and fingerprint', () => {
    const peer = { name: 'Pixel 8', os: 'android', fingerprint: 'C4D2·7781' };
    expect(isBridgeEvent({ kind: 'pair_peer_found', peer })).toBe(true);
    expect(isBridgeEvent({ kind: 'pair_peer_found', peer: { name: 'Pixel 8' } })).toBe(false);
    expect(isBridgeEvent({ kind: 'pair_peer_found', peer: { ...peer, fingerprint: 7 } })).toBe(
      false,
    );
  });
});
