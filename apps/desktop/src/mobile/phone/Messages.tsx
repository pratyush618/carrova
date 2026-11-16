import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { SyncWindow } from '../../bindings/SyncWindow';
import { usePort } from '../../engine/port';
import { Group } from '../../screens/settings/Group';
import { Banner, Button, Row, Segmented } from '../../ui';
import { PhonePage } from '../PhonePage';
import { usePermSteps } from './usePermSteps';
import '../../screens/settings/settings.css';

const SMS = ['sms'] as const;
const WINDOWS: readonly SyncWindow[] = ['days30', 'year', 'all'];

/**
 * Texts on the linked computers: whether Carrova may read them, how far
 * back they reach ("Keep in sync"), and why chat (RCS) messages come as
 * notifications instead.
 */
export function Messages({
  name,
  onBack,
  onAllow,
  onNotifications,
}: {
  /** The computer whose page this was opened from. */
  readonly name: string;
  readonly onBack: () => void;
  /** Grant the SMS permission. */
  readonly onAllow: () => void;
  /** Open the per-app notification list. */
  readonly onNotifications: () => void;
}) {
  const { t } = useTranslation();
  const port = usePort();
  const [keep, setKeep] = useState<SyncWindow | null>(null);
  const steps = usePermSteps(SMS);
  const noPerm = steps !== null && steps[0] !== 'ok';
  useEffect(() => {
    void port.messagesSync().then(setKeep);
  }, [port]);
  return (
    <PhonePage title={t('mobile.messages.title')} onBack={onBack}>
      {noPerm ? (
        <Banner
          tone="warning"
          icon="lock"
          title={t('mobile.messages.noPerm')}
          body={t('mobile.messages.noPermBody')}
          action={
            <Button size="sm" onClick={onAllow}>
              {t('mobile.messages.allow')}
            </Button>
          }
        />
      ) : (
        steps !== null &&
        keep !== null && (
          <Banner
            tone="success"
            title={t('mobile.messages.synced')}
            body={t(`mobile.messages.syncedBody.${keep}`, { name })}
          />
        )
      )}
      <Group title={t('mobile.messages.on', { name })}>
        <Row
          off={noPerm}
          title={t('mobile.messages.keep')}
          trail={
            <Segmented<SyncWindow>
              label={t('mobile.messages.keep')}
              options={WINDOWS.map((value) => ({
                value,
                label: t(`mobile.messages.keepOptions.${value}`),
              }))}
              value={keep}
              onChange={(next) => {
                setKeep(next);
                void port.setMessagesSync(next);
              }}
            />
          }
        />
      </Group>
      <Banner
        tone="accent"
        title={t('mobile.messages.rcs')}
        body={t('mobile.messages.rcsBody', { name })}
      />
      <Button onClick={onNotifications}>{t('mobile.messages.notifications')}</Button>
    </PhonePage>
  );
}
