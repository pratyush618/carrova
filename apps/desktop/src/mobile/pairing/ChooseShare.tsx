import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { LinkedDeviceView } from '../../engine/types';
import { FeatureRow } from '../../screens/devices/FeatureRow';
import { PHASE2_FEATURES, type Phase2Feature } from '../../screens/devices/model';
import { Group } from '../../screens/settings/Group';
import { Button } from '../../ui';
import { shareDefaults, shareRow, type ShareChoices } from '../model';
import { BatteryRow } from '../phone/BatteryRow';
import { PhonePage } from '../PhonePage';
import '../../screens/settings/settings.css';

/**
 * Right after linking: the Phase 2 features with `device`, clipboard, ring
 * and notifications preselected; Done saves them. Turning a feature on
 * asks for the Android permissions it needs (`onTurnOn`).
 */
export function ChooseShare({
  device,
  onTurnOn,
  onDone,
}: {
  readonly device: LinkedDeviceView;
  readonly onTurnOn: (feature: Phase2Feature) => void;
  readonly onDone: (choices: ShareChoices) => void;
}) {
  const { t } = useTranslation();
  const [choices, setChoices] = useState(shareDefaults);
  const rows = PHASE2_FEATURES.map((feature) => ({
    feature,
    row: shareRow(device, feature, choices[feature]),
  }));
  const group = (kind: 'quiet' | 'ask') =>
    rows
      .filter(({ row }) => row.group === kind)
      .map(({ feature, row }) => (
        <FeatureRow
          key={feature}
          feature={feature}
          row={row}
          device={device.name}
          off={false}
          body={t(`mobile.features.${feature}`)}
          onChange={(state) => {
            setChoices((now) => ({ ...now, [feature]: state }));
            if (state === 'on') onTurnOn(feature);
          }}
        />
      ));
  return (
    <PhonePage title={t('mobile.choose.title')}>
      <p>{t('mobile.choose.lede', { name: device.name })}</p>
      <Group title={t('ecosystem.groups.quiet')}>{group('quiet')}</Group>
      <Group title={t('ecosystem.groups.ask')} sub={t('ecosystem.askFoot')}>
        {group('ask')}
      </Group>
      <Group title={t('mobile.choose.background')}>
        <BatteryRow />
      </Group>
      <Button
        kind="primary"
        size="lg"
        onClick={() => {
          onDone(choices);
        }}
      >
        {t('mobile.choose.done')}
      </Button>
    </PhonePage>
  );
}
