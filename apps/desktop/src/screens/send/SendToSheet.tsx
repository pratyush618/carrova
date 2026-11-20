import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { AppGlyph, Badge, Button, Empty, GroupHead, List, Note, Row, Sheet } from '../../ui';
import { deviceIcon } from '../devices/model';
import { useDeviceSub } from '../nearby/DeviceCard';
import type { DeviceEntry } from '../nearby/model';
import { countsText } from '../shared/requestCounts';
import type { SendToIntents, SendToModel } from './model';

/**
 * After picking what moves: trusted computers first, each sent to without a
 * code; away ones greyed; new ones need a code once. Phones never show.
 */
export function SendToSheet({
  model,
  on,
}: {
  readonly model: SendToModel;
  readonly on: SendToIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const deviceSub = useDeviceSub();
  const { trusted, away, fresh } = model.targets;
  const row = (device: DeviceEntry, trail: ReactNode, off = false) => {
    const sub = deviceSub(device, model.now);
    return (
      <Row
        key={device.key}
        off={off}
        lead={
          <AppGlyph glyph={{ icon: deviceIcon(device.os) }} tone={off ? 'neutral' : 'accent'} />
        }
        title={
          <>
            {device.name}
            {device.kind === 'linked' && <Badge tone="success">{t('request.to.linked')}</Badge>}
            {off && <Badge tone="neutral">{t('request.to.away')}</Badge>}
            {device.kind === 'new' && <Badge tone="warning">{t('nearby.new')}</Badge>}
          </>
        }
        sub={
          device.kind === 'new'
            ? [sub, t('request.to.freshSub')].filter((part) => part !== '').join(' · ')
            : sub
        }
        trail={trail}
      />
    );
  };
  const button = (label: string, name: string, act: () => void, primary = false) => (
    <Button size="sm" kind={primary ? 'primary' : 'secondary'} aria-label={name} onClick={act}>
      {label}
    </Button>
  );
  const useCode = button(t('request.to.useCode'), t('request.to.useCode'), on.onUseCode);
  const nobody = trusted.length === 0 && away.length === 0;
  return (
    <Sheet
      title={t('request.to.title')}
      {...(model.summary !== null && { sub: countsText(model.summary, t, format) })}
      onClose={on.onCancel}
      foot={<Button onClick={on.onCancel}>{t('action.cancel')}</Button>}
    >
      {nobody ? (
        <Empty
          art="laptop"
          title={t('request.to.emptyTitle')}
          body={t('request.to.emptyBody')}
          action={
            <Button kind="primary" onClick={on.onUseCode}>
              {t('request.to.withCode')}
            </Button>
          }
        />
      ) : (
        <section>
          <GroupHead title={t('request.to.trusted')} />
          <List label={t('request.to.trusted')}>
            {trusted.map((d) =>
              row(
                d,
                button(
                  t('request.to.send'),
                  t('request.to.sendTo', { name: d.name }),
                  () => {
                    on.onSend(d);
                  },
                  true,
                ),
              ),
            )}
            {away.map((d) =>
              row(
                d,
                d.canWake
                  ? button(t('request.to.wake'), t('request.to.wakeName', { name: d.name }), () => {
                      on.onWake(d);
                    })
                  : useCode,
                true,
              ),
            )}
          </List>
        </section>
      )}
      {fresh.length > 0 && (
        <section>
          <GroupHead title={t('request.to.fresh')} />
          <List label={t('request.to.fresh')}>{fresh.map((d) => row(d, useCode))}</List>
          <Note>{t('request.to.freshFoot')}</Note>
        </section>
      )}
    </Sheet>
  );
}
