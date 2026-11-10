import { FlowLayout } from '../../ui';
import { TransferActions } from '../shared/TransferActions';
import { TransferBody, TransferRail, TransferRoute } from '../shared/TransferBody';
import type { TransferIntents } from '../shared/transfer';
import type { LiveModel } from './model';

export interface SendLiveScreenProps {
  /** The transfer itself (pairing and their review are `SendLinkedScreen`). */
  readonly model: Extract<LiveModel, { status: 'transfer' }>;
  readonly on: TransferIntents;
}

/** Sending: the live card and apps ticking off, with the move on the rail. */
export function SendLiveScreen({ model, on }: SendLiveScreenProps) {
  return (
    <FlowLayout
      rail={<TransferRail model={model.transfer} />}
      route={<TransferRoute model={model.transfer} />}
      actions={<TransferActions model={model.transfer} on={on} />}
    >
      <TransferBody model={model.transfer} on={on} />
    </FlowLayout>
  );
}
