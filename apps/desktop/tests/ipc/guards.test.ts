import { describe, expect, it } from 'vitest';
import type { ErrorCode } from '@/bindings/ErrorCode';
import { isBridgeError, toBridgeError } from '@/ipc/guards';

describe('the IPC guards', () => {
  it('keep every error code the engine sends, including the newest', () => {
    const codes: ErrorCode[] = ['invalid_selection', 'rolled_back', 'rollback_failed'];
    for (const code of codes) {
      const error = { code, message: 'm', hint: null };
      expect(isBridgeError(error)).toBe(true);
      expect(toBridgeError(error)).toEqual(error);
    }
  });

  it('turn an unknown code into an internal error', () => {
    expect(toBridgeError({ code: 'mystery', message: 'm', hint: null }).code).toBe('internal');
  });
});
