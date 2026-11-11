import { describe, expect, it } from 'vitest';
import type { ErrorCode } from '@/bindings/ErrorCode';
import { isBridgeError, isBridgeEvent, toBridgeError } from '@/ipc/guards';

describe('the IPC guards', () => {
  it('keep every error code the engine sends, including the newest', () => {
    const codes: ErrorCode[] = ['invalid_selection', 'rolled_back', 'rollback_failed'];
    for (const code of codes) {
      const error = { code, message: 'm', hint: null };
      expect(isBridgeError(error)).toBe(true);
      expect(toBridgeError(error)).toEqual(error);
    }
  });

  it('accept a send request, which names no session, but no other sessionless event', () => {
    expect(isBridgeEvent({ kind: 'send_requested' })).toBe(true);
    expect(isBridgeEvent({ kind: 'task_ended', error: null })).toBe(false);
  });

  it('accept a notice, which names no session, only with its text', () => {
    const notice = { id: 1, kind: 'move_finished', at: 1, read: false, peer: null, summary: 's' };
    expect(isBridgeEvent({ kind: 'notice', notice })).toBe(true);
    expect(isBridgeEvent({ kind: 'notice' })).toBe(false);
  });

  it('turn an unknown code into an internal error', () => {
    expect(toBridgeError({ code: 'mystery', message: 'm', hint: null }).code).toBe('internal');
  });
});
