import { useTranslation } from 'react-i18next';
import { usePort } from '../../engine/port';
import { useAsks } from '../../flows/ecosystem/useAsks';
import { useTransfers } from '../../flows/ecosystem/useTransfers';
import { AskToast } from '../../screens/asks/AskToast';
import { ClipToast } from '../../screens/asks/ClipToast';
import { TransferToast } from '../../screens/asks/TransferToast';
import { Toast } from '../../ui';
import { useControllers } from '../controllers/Controllers';
import { useEngineDispatch, useLink } from '../state/EngineState';

/** Linked devices' notes over any page: the oldest waiting offer, the newest file, a clip kept here. */
export function EcosystemToasts() {
  const { t } = useTranslation();
  const { linked } = useControllers();
  const dispatch = useEngineDispatch();
  const { clip } = useLink();
  const port = usePort();
  const { ask, answer, failed, dismissFailure } = useAsks(port);
  const { transfer, dismiss } = useTransfers();
  const name = (id: string) =>
    linked.devices.find((d) => d.id === id)?.name ?? t('ecosystem.unknownDevice');
  return (
    <>
      {ask !== null && (
        <AskToast key={ask.id} device={name(ask.device)} kind={ask.kind} onAnswer={answer} />
      )}
      {failed !== null && (
        <Toast icon="warn" onDismiss={dismissFailure}>
          {t('ecosystem.ask.failed', { name: name(failed) })}
        </Toast>
      )}
      {transfer !== null && (
        <TransferToast device={name(transfer.device)} transfer={transfer} onDismiss={dismiss} />
      )}
      {clip !== null && (
        <ClipToast
          key={clip.n}
          note={clip.note}
          device={clip.note.kind === 'received' ? name(clip.note.device) : ''}
          onDismiss={() => {
            dispatch({ type: 'clip_dismissed' });
          }}
        />
      )}
    </>
  );
}
