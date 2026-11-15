import { useCallback, useState } from 'react';
import { useEngineDispatch, useLink } from '../../app/state/EngineState';
import type { Transfer } from '../../app/state/appState';
import { toBridgeError } from '../../engine/errors';
import type { CarrovaPort } from '../../engine/types';

/** A retry that did not go: for which device, and whether because it is still away. */
export interface RetryFailure {
  readonly device: string;
  readonly away: boolean;
}

/**
 * The newest file transfer not dismissed. Retry sends it again as a new
 * transfer, which shows in its place. A retry that fails says so in `retryFailed`.
 */
export function useTransfers(port: CarrovaPort): {
  readonly transfer: Transfer | null;
  readonly dismiss: () => void;
  readonly retry: () => void;
  readonly retryFailed: RetryFailure | null;
  readonly dismissRetryFailure: () => void;
} {
  const transfer = useLink().transfers.findLast((t) => !t.dismissed) ?? null;
  const dispatch = useEngineDispatch();
  const [retryFailed, setRetryFailed] = useState<RetryFailure | null>(null);
  const dismiss = useCallback(() => {
    if (transfer !== null)
      dispatch({ type: 'transfer_dismissed', device: transfer.device, id: transfer.id });
  }, [transfer, dispatch]);
  const retry = useCallback(() => {
    if (transfer === null) return;
    const { device, id } = transfer;
    // Its outcome arrives as the new transfer's events, or as `retryFailed`.
    port.retryFile(id).catch((reason: unknown) => {
      // `network`: the device is not linked (yet) again.
      setRetryFailed({ device, away: toBridgeError(reason).code === 'network' });
    });
  }, [transfer, port]);
  const dismissRetryFailure = useCallback(() => {
    setRetryFailed(null);
  }, []);
  return { transfer, dismiss, retry, retryFailed, dismissRetryFailure };
}
