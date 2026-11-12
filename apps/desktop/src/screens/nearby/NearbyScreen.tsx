import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { Banner, Button, HubLayout, Segmented, Toast, type SegmentOption } from '../../ui';
import { DeviceCard } from './DeviceCard';
import type { NearbyIntents, NearbyModel, NearbyView } from './model';
import { NearbyList } from './NearbyList';
import { NearbyMap } from './NearbyMap';
import { VisibilityPill } from './VisibilityPill';
import './nearby.css';

const VIEWS: readonly NearbyView[] = ['map', 'list'];

/** Nobody answered yet: what to check, and the code that always works. */
function Looking({ onSendWithCode }: { readonly onSendWithCode: () => void }) {
  const { t } = useTranslation();
  return (
    <div className="s-near__looking" role="status">
      <h2>{t('nearby.empty.title')}</h2>
      <p>{t('nearby.empty.body')}</p>
      <Button kind="primary" size="sm" onClick={onSendWithCode}>
        {t('nearby.empty.action')}
      </Button>
    </div>
  );
}

/**
 * Carrova computers on this network, as a map or a list. The header bar
 * switches the view; who can see this computer shows once, in the pill.
 */
export function NearbyScreen({
  model,
  on,
}: {
  readonly model: NearbyModel;
  readonly on: NearbyIntents;
}) {
  const { t } = useTranslation();
  // The scan pulse plays once per page open, not again on returning to the map.
  const [pulse, setPulse] = useState(true);
  if (pulse && model.view !== 'map') setPulse(false);
  const views: SegmentOption<NearbyView>[] = VIEWS.map((value) => ({
    value,
    label: t(`nearby.views.${value}`),
  }));
  const devices = [...model.here, ...model.away];
  const selected = devices.find((device) => device.key === model.selected) ?? null;
  const looking = model.here.length === 0 && <Looking onSendWithCode={on.onSendWithCode} />;
  const wakeFailed = model.wake?.outcome === 'sent' ? null : model.wake;
  return (
    <HubLayout
      variant="flush"
      tools={
        <Segmented
          label={t('nearby.show')}
          options={views}
          value={model.view}
          onChange={on.onView}
        />
      }
    >
      <div className="s-near">
        <h1 className="p-vh">{t('nearby.title')}</h1>
        {model.view === 'map' ? (
          <NearbyMap
            devices={devices}
            selected={model.selected}
            pulse={pulse}
            onPulsed={() => {
              setPulse(false);
            }}
            onSelect={on.onSelect}
          >
            {selected === null ? looking : <DeviceCard device={selected} now={model.now} on={on} />}
          </NearbyMap>
        ) : (
          <NearbyList here={model.here} away={model.away} now={model.now} on={on}>
            {looking}
          </NearbyList>
        )}
        {model.visibility !== null && (
          <VisibilityPill
            visibility={model.visibility}
            me={model.me}
            onChoose={on.onVisibility}
            onSettings={on.onSettings}
          />
        )}
        {model.wake?.outcome === 'sent' && (
          <Toast onDismiss={on.onDismissWake}>
            {t('nearby.wake.sent', { name: model.wake.name })}
          </Toast>
        )}
        {(model.failed !== null || wakeFailed !== null) && (
          <div className="s-near__note">
            {model.failed !== null && (
              <Banner tone="warning" title={t(`nearby.failed.${model.failed}`)} />
            )}
            {wakeFailed !== null && (
              <Banner
                tone="warning"
                title={t(`nearby.wake.${wakeFailed.outcome}`, { name: wakeFailed.name })}
                action={
                  <Button kind="quiet" size="sm" onClick={on.onDismissWake}>
                    {t('ui.dismiss')}
                  </Button>
                }
              />
            )}
          </div>
        )}
      </div>
    </HubLayout>
  );
}
