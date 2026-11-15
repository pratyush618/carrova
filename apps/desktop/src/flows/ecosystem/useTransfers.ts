import { useCallback } from 'react';
import { useEngineDispatch, useLink } from '../../app/state/EngineState';
import type { Transfer } from '../../app/state/appState';
import type { CarrovaPort } from '../../engine/types';

/**
 * The newest file transfer not dismissed. Retry sends it again as a new
 * transfer, which then shows in its place; a retry that cannot start
 * (still away) leaves it, Retry and all.
 */
export function useTransfers(port: CarrovaPort): {
  readonly transfer: Transfer | null;
  readonly dismiss: () => void;
  readonly retry: () => void;
} {
  const transfer = useLink().transfers.findLast((t) => !t.dismissed) ?? null;
  const dispatch = useEngineDispatch();
  const dismiss = useCallback(() => {
    if (transfer !== null)
      dispatch({ type: 'transfer_dismissed', device: transfer.device, id: transfer.id });
  }, [transfer, dispatch]);
  const retry = useCallback(() => {
    // Its outcome arrives as the new transfer's events.
    if (transfer !== null) void port.retryFile(transfer.id).catch(() => undefined);
  }, [transfer, port]);
  return { transfer, dismiss, retry };
}
