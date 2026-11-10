import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  ActionBar,
  Banner,
  Button,
  Empty,
  FlowLayout,
  Head,
  LinkButton,
  Note,
  Progress,
  Status,
} from '../../ui';
import { FoundSelection, selectionTotals } from './FoundSelection';
import type { FoundIntents, FoundModel, FoundPurpose } from './model';

export interface SendFoundScreenProps {
  readonly model: FoundModel;
  readonly purpose: FoundPurpose;
  readonly on: FoundIntents;
}

/** "Here's what we found": the one selection screen for sending and for saving a file. */
export function SendFoundScreen({ model, purpose, on }: SendFoundScreenProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const back = (
    <Button icon="back" onClick={on.onBack}>
      {t('action.back')}
    </Button>
  );

  switch (model.status) {
    case 'scanning':
      return (
        <FlowLayout
          actions={<ActionBar start={<Button onClick={on.onBack}>{t('action.cancel')}</Button>} />}
        >
          <Head title={t('send.found.scanningTitle')} lede={t('send.found.scanningLede')} />
          <Status>
            {model.foundSoFar.length > 0
              ? t('send.found.foundSoFar', { apps: model.foundSoFar.join(', ') })
              : t('send.found.looking')}
          </Status>
          {model.progress !== null && (
            <Progress value={model.progress} label={t('send.found.scanProgress')} />
          )}
        </FlowLayout>
      );
    case 'nothing':
      return (
        <FlowLayout actions={<ActionBar start={back} />}>
          <Empty
            art="search"
            title={t('send.found.nothingTitle')}
            body={t('send.found.nothingBody', { count: model.knownApps })}
            action={
              <Button kind="primary" onClick={on.onPickFolders}>
                {t('send.found.pickFolders')}
              </Button>
            }
          />
          <Note center>
            <LinkButton onClick={on.onSeeKnownApps}>{t('send.found.seeKnown')}</LinkButton>
          </Note>
        </FlowLayout>
      );
    case 'found': {
      const size = format.bytes(selectionTotals(model.apps).bytes);
      const withKeys = model.asks.some((a) => a.kind === 'secret' && a.answer === 'bring');
      return (
        <FlowLayout
          actions={
            <ActionBar
              start={<Button onClick={on.onCustomize}>{t('send.found.customize')}</Button>}
              note={
                withKeys
                  ? t('send.found.noteWithKeys', { size })
                  : t('send.found.noteNoKeys', { size })
              }
              end={
                <Button kind="primary" disabled={model.issues.length > 0} onClick={on.onContinue}>
                  {purpose === 'save' ? t('action.continue') : t('send.found.toPairing')}
                </Button>
              }
            />
          }
        >
          <FoundSelection apps={model.apps} asks={model.asks} on={on} />
          {model.issues.map((issue) => (
            <Banner key={issue} tone="danger" title={t('send.found.issue')} body={issue} />
          ))}
        </FlowLayout>
      );
    }
  }
}
