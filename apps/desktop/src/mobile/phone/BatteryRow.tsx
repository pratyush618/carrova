import { useTranslation } from 'react-i18next';
import { usePort } from '../../engine/port';
import { Button, Row } from '../../ui';
import { usePermSteps } from './usePermSteps';

const BATTERY = ['battery'] as const;

/**
 * "Stay connected when closed": Allow opens Android's own request to stop
 * optimising Carrova's battery; the answer is read back on return.
 */
export function BatteryRow() {
  const { t } = useTranslation();
  const port = usePort();
  const steps = usePermSteps(BATTERY);
  const ok = steps?.[0] === 'ok';
  return (
    <Row
      title={t('mobile.choose.stay')}
      sub={t(ok ? 'mobile.choose.stayOk' : 'mobile.choose.staySub')}
      {...(steps !== null &&
        !ok && {
          trail: (
            <Button
              size="sm"
              onClick={() => {
                void port.openPermSettings('battery');
              }}
            >
              {t('mobile.choose.allow')}
            </Button>
          ),
        })}
    />
  );
}
