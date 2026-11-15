import { useCallback } from 'react';
import { useEngineDispatch, useLink } from '../../app/state/EngineState';
import type { Transfer } from '../../app/state/appState';

/** The newest file transfer, until dismissed. */
export function useTransfers(): {
  readonly transfer: Transfer | null;
  readonly dismiss: () => void;
} {
  const transfer = useLink().transfers.at(-1) ?? null;
  const dispatch = useEngineDispatch();
  const dismiss = useCallback(() => {
    if (transfer !== null)
      dispatch({ type: 'transfer_dismissed', device: transfer.device, id: transfer.id });
  }, [transfer, dispatch]);
  return { transfer, dismiss };
}
