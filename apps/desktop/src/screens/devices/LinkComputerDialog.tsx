import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { Button, Dialog, Field, Input } from '../../ui';
import { LinkedSummary } from './LinkedSummary';

export type LinkComputerState = 'typing' | 'linking' | 'failed' | 'linked';

export interface LinkComputerIntents {
  readonly onCode: (code: string) => void;
  readonly onLink: () => void;
  readonly onCancel: () => void;
  /** Open the new computer's page. */
  readonly onOpen: () => void;
}

/** Link a computer: type the code the other one shows; linking needs no review (nothing moves yet). */
export function LinkComputerDialog({
  code,
  state,
  device = null,
  on,
}: {
  readonly code: string;
  readonly state: LinkComputerState;
  /** The computer that linked, once the list names it. */
  readonly device?: LinkedDeviceView | null;
  readonly on: LinkComputerIntents;
}) {
  const { t } = useTranslation();
  if (state === 'linked')
    return (
      <Dialog
        icon="check"
        title={t('ecosystem.linkComputer.linkedTitle')}
        onDismiss={on.onCancel}
        foot={
          <>
            <Button onClick={on.onCancel}>{t('ecosystem.pair.done')}</Button>
            <Button kind="primary" data-initial-focus onClick={on.onOpen}>
              {t('ecosystem.pair.open')}
            </Button>
          </>
        }
      >
        <LinkedSummary device={device} />
      </Dialog>
    );
  const linking = state === 'linking';
  return (
    <Dialog
      icon="laptop"
      title={t('ecosystem.linkComputer.title')}
      onDismiss={on.onCancel}
      foot={
        <>
          <Button onClick={on.onCancel}>{t('ecosystem.pair.cancel')}</Button>
          <Button
            kind="primary"
            busy={linking}
            disabled={linking || code.trim() === ''}
            onClick={on.onLink}
          >
            {linking ? t('ecosystem.linkComputer.linking') : t('ecosystem.linkComputer.link')}
          </Button>
        </>
      }
    >
      <p>{t('ecosystem.linkComputer.lede')}</p>
      <Field
        label={t('ecosystem.linkComputer.field')}
        {...(state === 'failed' && { error: t('ecosystem.linkComputer.failed') })}
      >
        {(control) => (
          <Input
            {...control}
            mono
            data-initial-focus
            autoComplete="off"
            spellCheck={false}
            value={code}
            disabled={linking}
            onChange={(event) => {
              on.onCode(event.target.value);
            }}
            onKeyDown={(event) => {
              if (event.key === 'Enter' && code.trim() !== '') on.onLink();
            }}
          />
        )}
      </Field>
    </Dialog>
  );
}
