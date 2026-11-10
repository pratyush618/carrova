import { useTranslation } from 'react-i18next';
import { errorText } from '../../engine/errors';
import { Button, Dialog, Disclose, List, Log, LockLine, Note, Row, Sheet, Status } from '../../ui';
import type {
  AboutIntents,
  AboutModel,
  CrashIntents,
  CrashModel,
  UpdateIntents,
  UpdateModel,
} from './model';

function useUpdateText(model: UpdateModel): { title: string; body: string | null } {
  const { t } = useTranslation();
  switch (model.status) {
    case 'checking':
      return { title: t('settings.update.checking'), body: null };
    case 'up_to_date':
      return { title: t('settings.more.upToDate'), body: null };
    case 'available':
      return { title: t('settings.update.title', { version: model.version }), body: model.notes };
    case 'installing':
      return { title: t('settings.update.installing', { version: model.version }), body: null };
    case 'installed':
      return { title: t('settings.update.installed', { version: model.version }), body: null };
    case 'failed':
      return { title: errorText(model.error, t).title, body: errorText(model.error, t).body };
  }
}

/** App updates: offered, installed only on the user's click, then a restart they choose. */
export function UpdateDialog({
  model,
  on,
}: {
  readonly model: UpdateModel;
  readonly on: UpdateIntents;
}) {
  const { t } = useTranslation();
  const text = useUpdateText(model);
  const primary =
    model.status === 'available' ? (
      <Button kind="primary" onClick={on.onInstall}>
        {t('settings.update.install')}
      </Button>
    ) : model.status === 'installed' ? (
      <Button kind="primary" onClick={on.onRestart}>
        {t('settings.update.restart')}
      </Button>
    ) : null;
  return (
    <Dialog
      icon="spark"
      title={text.title}
      onDismiss={on.onLater}
      foot={
        <>
          <Button data-initial-focus onClick={on.onLater}>
            {primary === null ? t('ui.close') : t('action.later')}
          </Button>
          {primary}
        </>
      }
    >
      {(model.status === 'checking' || model.status === 'installing') && (
        <Status>{t('settings.update.wait')}</Status>
      )}
      {text.body !== null && <p>{text.body}</p>}
      {(model.status === 'available' || model.status === 'installed') && (
        <LockLine>{t('settings.update.verified')}</LockLine>
      )}
    </Dialog>
  );
}

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
