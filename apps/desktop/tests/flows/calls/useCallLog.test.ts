import { act, renderHook, waitFor } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';
import type { CallLogView } from '@/bindings/CallLogView';
import type { CallRow } from '@/bindings/CallRow';
import { appendPage, useCallLog } from '@/flows/calls/useCallLog';
import { createFakePort } from '@/ipc/fake';

const row = (id: number): CallRow => ({
  id,
  caller: { number: '+1', name: null },
  when_ms: id,
  duration_s: 1,
  kind: 'incoming',
});

/** A page of one call, `id`, that has older ones after it. */
const page = (id: number): CallLogView => ({
  groups: [{ days_ago: 0, rows: [row(id)] }],
  next: { when_ms: id, id },
});

describe('appendPage', () => {
  it('joins a day split across two pages', () => {
    const first = { groups: [{ days_ago: 0, rows: [row(5)] }], next: { when_ms: 5, id: 5 } };
    const older = {
      groups: [
        { days_ago: 0, rows: [row(4)] },
        { days_ago: 2, rows: [row(3)] },
      ],
      next: null,
    };
    expect(appendPage(first, older)).toEqual({
      groups: [
        { days_ago: 0, rows: [row(5), row(4)] },
        { days_ago: 2, rows: [row(3)] },
      ],
      next: null,
    });
  });
});

describe('useCallLog', () => {
  afterEach(() => {
    vi.restoreAllMocks();
  });

  /** The hook over a port whose first page for any phone is `page(9)`. */
  async function loaded() {
    const port = createFakePort({ stepMs: 0 });
    const history = vi.spyOn(port, 'callHistory').mockResolvedValue(page(9));
    const hook = renderHook(
      (props: { id: string; on: boolean }) => useCallLog(port, props.id, props.on),
      { initialProps: { id: 'p8', on: true } },
    );
    await waitFor(() => {
      expect(hook.result.current.log).not.toBeNull();
    });
    return { history, ...hook };
  }

  it('asks for no older page once calls are off', async () => {
    const { history, result, rerender } = await loaded();
    rerender({ id: 'p8', on: false });
    act(() => {
      result.current.more();
    });
    expect(history).toHaveBeenCalledTimes(1);
  });

  it('clears a failed read once the phone answers again', async () => {
    const port = createFakePort({ stepMs: 0 });
    const history = vi.spyOn(port, 'callHistory').mockRejectedValue(new Error('refused'));
    const { result, rerender } = renderHook((on: boolean) => useCallLog(port, 'p8', on), {
      initialProps: true,
    });
    await waitFor(() => {
      expect(result.current.failed).toBe(true);
    });
    history.mockResolvedValue(page(9));
    rerender(false);
    rerender(true);
    await waitFor(() => {
      expect(result.current.log).toEqual(page(9));
    });
    expect(result.current.failed).toBe(false);
  });

  it('asks for an older page once while it is on its way', async () => {
    const { history, result } = await loaded();
    history.mockReturnValue(new Promise(() => undefined));
    act(() => {
      result.current.more();
      result.current.more();
    });
    expect(history).toHaveBeenCalledTimes(2);
  });

  it('reads older pages against the first page’s clock', async () => {
    const now = vi.spyOn(Date, 'now').mockReturnValue(1_000);
    const { history, result } = await loaded();
    now.mockReturnValue(1_000 + 86_400_000);
    act(() => {
      result.current.more();
    });
    expect(history.mock.calls[1]?.[2]).toEqual(history.mock.calls[0]?.[2]);
  });

  it('drops an older page that lands after the log restarted', async () => {
    const { history, result, rerender } = await loaded();
    let finish: (value: CallLogView) => void = () => undefined;
    history.mockReturnValueOnce(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    act(() => {
      result.current.more();
    });
    history.mockResolvedValue(page(7));
    rerender({ id: 'd2', on: true });
    await waitFor(() => {
      expect(result.current.log).toEqual(page(7));
    });
    await act(async () => {
      finish(page(8));
      await Promise.resolve();
    });
    expect(result.current.log).toEqual(page(7));
  });
});
