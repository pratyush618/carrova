import { act, renderHook } from '@testing-library/react';
import { describe, expect, it } from 'vitest';
import { useCommand } from '@/flows/useRemote';

/** A call whose result the test releases by hand. */
function deferred<T>() {
  let resolve: (value: T) => void = () => undefined;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

describe('useCommand', () => {
  it('lets only the newest of overlapping calls land', async () => {
    const calls = [deferred<string>(), deferred<string>()];
    let n = 0;
    const call = () => (calls[n++] ?? deferred<string>()).promise;
    const { result } = renderHook(() => useCommand(call));
    let first: Promise<string | null> = Promise.resolve(null);
    act(() => {
      first = result.current.run();
      void result.current.run();
    });
    await act(async () => {
      calls[1]?.resolve('new');
      calls[0]?.resolve('old');
      await first;
    });
    expect(await first).toBeNull();
    expect(result.current.state).toEqual({ status: 'ready', value: 'new' });
  });

  it('turns a rejection into a BridgeError', async () => {
    const { result } = renderHook(() => useCommand(() => Promise.reject(new Error('boom'))));
    await act(async () => {
      await result.current.run();
    });
    expect(result.current.state).toMatchObject({
      status: 'failed',
      error: { code: 'internal', message: 'boom' },
    });
  });
});
