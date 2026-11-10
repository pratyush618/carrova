import type { TFunction } from 'i18next';
import { interrupted, type TransferApp, type TransferModel } from './transfer';

/**
 * What a chip says about its app. Pure, so the wording rules (the moving app
 * carries the byte count; applying says "in place") live in one place.
 */
export function chipText(
  app: TransferApp,
  transfer: TransferModel,
  t: TFunction,
  bytes: (n: number) => string,
): string {
  const applying = transfer.phase === 'applying';
  switch (app.state) {
    case 'done':
      return applying ? t('transfer.chip.inPlace') : t('transfer.chip.arrived');
    case 'failed':
      return t('transfer.chip.failed');
    case 'waiting':
      return interrupted(transfer.phase) ? t('transfer.chip.paused') : t('transfer.chip.waiting');
    case 'moving':
      if (applying) return t('transfer.chip.placing');
      return app.bytesDone > 0
        ? t('summary.ofTotal', { done: bytes(app.bytesDone), total: bytes(app.bytes) })
        : t('transfer.chip.moving');
  }
}
