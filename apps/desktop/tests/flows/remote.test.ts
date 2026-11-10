import { describe, expect, it } from 'vitest';
import { current, idle, remoteReducer, type Remote } from '@/flows/remote';

const error = { code: 'network', message: 'Lost', hint: null } as const;

describe('remote values', () => {
  it('keeps the last value visible while reloading and after a failure', () => {
    const ready: Remote<number> = remoteReducer(idle, { type: 'ok', value: 3 });
    const loading = remoteReducer(ready, { type: 'start' });
    expect(current(loading)).toBe(3);
    const failed = remoteReducer(loading, { type: 'fail', error });
    expect(failed).toEqual({ status: 'failed', error, last: 3 });
  });

  it('forgets everything on reset', () => {
    expect(remoteReducer<number>({ status: 'ready', value: 1 }, { type: 'reset' })).toBe(idle);
  });
});
