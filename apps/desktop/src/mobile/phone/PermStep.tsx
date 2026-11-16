import { useTranslation } from 'react-i18next';
import type { PermStep as Step } from '../../bindings/PermStep';
import type { PhonePerm } from '../../bindings/PhonePerm';
import { Banner, Button } from '../../ui';
import { PhonePage } from '../PhonePage';

/**
 * The step one Android permission needs before a feature works: "Allow
 * restricted settings" (App info › ⋮), notification access (its own
 * settings page), or a runtime permission (granted in App info). Each
 * button opens that page; the app checks again on return.
 */
export function PermStep({
  perm,
  step,
  onOpen,
  onBack,
}: {
  readonly perm: PhonePerm;
  readonly step: Exclude<Step, 'ok'>;
  readonly onOpen: () => void;
  readonly onBack: () => void;
}) {
  const { t } = useTranslation();
  const open = (label: string) => (
    <Button kind="primary" size="lg" onClick={onOpen}>
      {label}
    </Button>
  );
  switch (step) {
    case 'restricted':
      return (
        <PhonePage title={t('mobile.perm.restricted.title')} onBack={onBack}>
          <Banner
            tone="warning"
            icon="lock"
            title={t('mobile.perm.restricted.warn')}
            body={t('mobile.perm.restricted.warnBody')}
          />
          <ol className="m-steps">
            <li>
              <b>{t('mobile.perm.restricted.step1')}</b>
              <span>{t('mobile.perm.restricted.step1Sub')}</span>
            </li>
            <li>
              <b>{t('mobile.perm.restricted.step2')}</b>
            </li>
            <li>
              <b>{t('mobile.perm.restricted.step3')}</b>
              <span>{t('mobile.perm.restricted.step3Sub')}</span>
            </li>
          </ol>
          {open(t('mobile.perm.restricted.open'))}
          <p className="m-note">{t('mobile.perm.restricted.foot')}</p>
        </PhonePage>
      );
    case 'open_settings':
      return (
        <PhonePage title={t('mobile.perm.access.title')} onBack={onBack}>
          <p>{t('mobile.perm.access.body')}</p>
          <p className="m-note">{t('mobile.perm.access.foot')}</p>
          {open(t('mobile.perm.access.open'))}
        </PhonePage>
      );
    case 'ask':
      return (
        <PhonePage title={t(`mobile.perm.ask.${perm}`)} onBack={onBack}>
          <p>{t(`mobile.perm.ask.why.${perm}`)}</p>
          <p className="m-note">{t('mobile.perm.ask.how')}</p>
          {open(t('mobile.perm.ask.open'))}
        </PhonePage>
      );
  }
}
