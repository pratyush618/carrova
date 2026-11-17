import { useState, type ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import type { Phase2Feature } from '../../screens/devices/model';
import { Button, List, Muted, Row, Segmented, Sheet } from '../../ui';
import { unreachable } from '../model';

/**
 * Send something to one computer: the linked computers `feature` applies
 * to, those it reaches now to pick from (the first one picked), the others
 * dimmed with why (off on this phone: turn it on here); `children` shows
 * what goes. The list follows `devices`, so a link that comes up shows.
 */
export function PickComputerSheet({
  title,
  feature,
  devices,
  onSend,
  onTurnOn,
  onClose,
  children,
}: {
  readonly title: string;
  readonly feature: Phase2Feature;
  /** The linked computers. */
  readonly devices: readonly LinkedDeviceView[];
  readonly onSend: (id: string) => void;
  /** Turn `feature` on for that computer on this phone. */
  readonly onTurnOn: (id: string) => void;
  readonly onClose: () => void;
  readonly children?: ReactNode;
}) {
  const { t } = useTranslation();
  const [picked, setPicked] = useState<string | null>(null);
  const applies = devices.filter((d) => d.features.some((f) => f.feature === feature));
  const ready = applies.filter((d) => unreachable(d, feature) === null);
  const out = applies.flatMap((device) => {
    const why = unreachable(device, feature);
    return why === null ? [] : [{ device, why }];
  });
  // The first one until the user picks one it still reaches (the list may change while open).
  const id = ready.find((d) => d.id === picked)?.id ?? ready[0]?.id ?? null;
  return (
    <Sheet
      title={title}
      onClose={onClose}
      foot={
        <>
          <Button onClick={onClose}>{t('mobile.pick.cancel')}</Button>
          <Button
            kind="primary"
            disabled={id === null}
            onClick={() => {
              if (id !== null) onSend(id);
            }}
          >
            {t('mobile.pick.send')}
          </Button>
        </>
      }
    >
      {children}
      {applies.length === 0 && <Muted>{t('mobile.home.noneNearby')}</Muted>}
      {ready.length > 0 && (
        <Segmented
          label={t('mobile.pick.to')}
          options={ready.map((d) => ({ value: d.id, label: d.name }))}
          value={id}
          onChange={setPicked}
        />
      )}
      {out.length > 0 && (
        <List label={t('mobile.pick.unreachable')}>
          {out.map(({ device, why }) => (
            <Row
              key={device.id}
              off
              title={device.name}
              sub={
                why === 'offHere' || why === 'offThere'
                  ? t(`mobile.pick.${why}`, { name: device.name })
                  : t(`ecosystem.status.${why}`)
              }
              trail={
                why === 'offHere' && (
                  <Button
                    size="sm"
                    onClick={() => {
                      onTurnOn(device.id);
                    }}
                  >
                    {t('mobile.pick.turnOn')}
                  </Button>
                )
              }
            />
          ))}
        </List>
      )}
    </Sheet>
  );
}
