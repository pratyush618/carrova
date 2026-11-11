import { toBridgeError } from '../../engine/errors';
import type { BridgeError, ScanResult, SessionId } from '../../engine/types';
import type { ByMode, SendFlowAction, SendMode } from './sendFlow';

/**
 * One queued request: `mode`'s next reply from its last one (`null`: unchanged).
 * `show` dispatches only while the request's session is still the current one.
 */
export type Request = (
  id: SessionId,
  base: ScanResult | null,
  show: (action: SendFlowAction) => void,
) => Promise<ScanResult | null>;

/** A session's selection requests, run one after another. */
export interface SelectionQueue {
  /**
   * Each mode's reply, once every queued request has answered. Each request
   * runs on its mode's result before, so the engine applies edits and picks in order.
   */
  latest: Promise<ByMode | null>;
  /** Each mode's newest request: only its outcome is shown. */
  readonly newest: Record<SendMode, Promise<unknown> | null>;
}

/** A queue holding `all` as each mode's reply. */
export const queueFrom = (all: ByMode | null): SelectionQueue => ({
  latest: Promise.resolve(all),
  newest: { auto: null, manual: null },
});

/** Where a request's outcome goes, and whether its session is still the current one. */
export interface Outlet {
  readonly dispatch: (action: SendFlowAction) => void;
  readonly live: () => boolean;
  /** How a failure shows; it changes nothing. */
  readonly failed: (error: BridgeError) => SendFlowAction;
}

/**
 * Queue `request` for `on` in session `id`. Resolves `true` once it succeeded
 * while its session was still live.
 */
export function enqueue(
  queue: SelectionQueue,
  on: SendMode,
  id: SessionId,
  request: Request,
  { dispatch, live, failed }: Outlet,
): Promise<boolean> {
  const show = (action: SendFlowAction) => {
    if (live()) dispatch(action);
  };
  let ok = false;
  const queued: Promise<ByMode | null> = queue.latest.then(async (all) => {
    if (all === null) return null;
    try {
      const result = await request(id, all[on], show);
      ok = live();
      if (result === null) return all;
      if (queue.newest[on] === queued) show({ type: 'selected', mode: on, result });
      return { ...all, [on]: result };
    } catch (reason: unknown) {
      if (queue.newest[on] === queued) show(failed(toBridgeError(reason)));
      // Refused requests change nothing: the engine keeps the previous selection.
      return all;
    }
  });
  queue.latest = queued;
  queue.newest[on] = queued;
  return queued.then(() => ok);
}
