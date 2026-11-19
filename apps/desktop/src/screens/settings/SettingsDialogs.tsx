import { useTranslation } from 'react-i18next';
import { Button, Dialog, Disclose, List, Log, Note, Row, Sheet } from '../../ui';
import type { AboutIntents, AboutModel, CrashIntents, CrashModel } from './model';

/** One error report, exactly as it would be sent. Sending opens a GitHub issue the user reviews. */
export function CrashDialog({
  model,
  on,
}: {
  readonly model: CrashModel;
  readonly on: CrashIntents;
}) {
  const { t } = useTranslation();
  return (
    <Dialog
      icon="warn"
      title={model.draft === null ? t('settings.crash.title') : model.draft.title}
      onDismiss={on.onDontSend}
      foot={
        <>
          {on.onDelete && (
            <Button kind="quiet" onClick={on.onDelete}>
              {t('settings.crash.delete')}
            </Button>
          )}
          <Button data-initial-focus onClick={on.onDontSend}>
            {t('settings.crash.dontSend')}
          </Button>
          <Button kind="primary" onClick={on.onSend}>
            {model.draft === null ? t('settings.crash.prepare') : t('settings.crash.send')}
          </Button>
        </>
      }
    >
      <p>{model.draft === null ? t('settings.crash.body') : t('settings.crash.draftBody')}</p>
      {model.draft === null ? (
        <Disclose summary={t('settings.crash.contents')}>
          <Log lines={model.report} />
        </Disclose>
      ) : (
        <Log lines={model.draft.body.split('\n')} />
      )}
    </Dialog>
  );
}

/** Version, licence, credits. */
export function AboutSheet({
  model,
  on,
}: {
  readonly model: AboutModel;
  readonly on: AboutIntents;
}) {
  const { t } = useTranslation();
  return (
    <Sheet
      title={t('settings.about.title')}
      onClose={on.onClose}
      foot={<Button onClick={on.onClose}>{t('ui.close')}</Button>}
    >
      <List>
        {model.version !== null && (
          <Row title={t('settings.about.version')} trail={model.version} />
        )}
        {model.engineVersion !== null && (
          <Row title={t('settings.about.engine')} trail={model.engineVersion} />
        )}
        {model.device !== null && <Row title={t('settings.about.device')} trail={model.device} />}
        {model.os !== null && (
          <Row title={t('settings.about.system')} trail={t(`settings.about.os.${model.os}`)} />
        )}
        <Row title={t('settings.about.licence')} trail={model.licence} />
        {on.onSource && (
          <Row
            title={t('settings.about.source')}
            trail={
              <Button kind="quiet" size="sm" onClick={on.onSource}>
                {t('action.open')}
              </Button>
            }
          />
        )}
      </List>
      <Note>{t('settings.about.credits')}</Note>
      {on.onThirdParty && (
        <div>
          <Button onClick={on.onThirdParty}>{t('settings.about.thirdParty')}</Button>
        </div>
      )}
    </Sheet>
  );
}
