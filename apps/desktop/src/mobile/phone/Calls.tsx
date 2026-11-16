import { useTranslation } from 'react-i18next';
import { Group } from '../../screens/settings/Group';
import { Banner, Button, Row } from '../../ui';
import { PhonePage } from '../PhonePage';
import { FEATURE_PERMS, pendingPerm } from './model';
import { usePermSteps } from './usePermSteps';
import '../../screens/settings/settings.css';

const PERMS = FEATURE_PERMS.calls ?? [];

/**
 * Calls on the linked computers: they ring when this phone does once
 * Carrova may see calls and contacts; using the computer's mic and
 * speakers is an optional step set up on the computer.
 */
export function Calls({
  name,
  onBack,
  onAllow,
}: {
  /** The computer whose page this was opened from. */
  readonly name: string;
  readonly onBack: () => void;
  /** Grant the calls and contacts permissions. */
  readonly onAllow: () => void;
}) {
  const { t } = useTranslation();
  const steps = usePermSteps(PERMS);
  const ready = steps !== null && pendingPerm(PERMS, steps) === null;
  return (
    <PhonePage title={t('mobile.calls.title')} onBack={onBack}>
      {ready ? (
        <Banner
          tone="success"
          title={t('mobile.calls.ready')}
          body={t('mobile.calls.readyBody', { name })}
        />
      ) : (
        <>
          <p>{t('mobile.calls.lede', { name })}</p>
          <ol className="m-steps">
            <li>
              <b>{t('mobile.calls.step1')}</b>
              <span>{t('mobile.calls.step1Sub', { name })}</span>
            </li>
            <li>
              <b>{t('mobile.calls.step2', { name })}</b>
              <span>{t('mobile.calls.step2Sub')}</span>
            </li>
          </ol>
          <Button kind="primary" size="lg" onClick={onAllow}>
            {t('mobile.calls.allow')}
          </Button>
        </>
      )}
      <Group title={t('mobile.calls.optional')}>
        <Row title={t('mobile.calls.audio', { name })} sub={t('mobile.calls.audioSub', { name })} />
      </Group>
    </PhonePage>
  );
}
