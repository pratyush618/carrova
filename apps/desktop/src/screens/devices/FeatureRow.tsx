import type { FeatureRowView, FeatureState } from '../../engine/types';
import { useTranslation } from 'react-i18next';
import { AppGlyph, Row, Segmented, Toggle } from '../../ui';
import { FEATURE_ICON, type Phase1Feature } from './model';

/**
 * One feature on one device: a switch for the quiet ones, Ask/Off for the
 * ones that ask (so "always allow" never exists by accident). When the other
 * side has it off, that replaces the description: it is why nothing flows.
 */
export function FeatureRow({
  feature,
  row,
  device,
  off,
  onChange,
}: {
  readonly feature: Phase1Feature;
  readonly row: FeatureRowView;
  readonly device: string;
  /** The device is away: dimmed, still changeable (the choice is kept). */
  readonly off: boolean;
  readonly onChange: (state: FeatureState) => void;
}) {
  const { t } = useTranslation();
  const title = t(`ecosystem.features.${feature}.title`);
  return (
    <Row
      off={off}
      lead={<AppGlyph glyph={{ icon: FEATURE_ICON[feature] }} size="sm" />}
      title={title}
      sub={
        row.remote === 'off'
          ? t('ecosystem.offOn', { name: device })
          : t(`ecosystem.features.${feature}.body`)
      }
      trail={
        row.group === 'quiet' ? (
          <Toggle
            on={row.local === 'on'}
            label={title}
            onChange={(on) => {
              onChange(on ? 'on' : 'off');
            }}
          />
        ) : (
          <Segmented<FeatureState>
            label={title}
            options={(['ask', 'off'] as const).map((value) => ({
              value,
              label: t(`ecosystem.states.${value}`),
            }))}
            value={row.local}
            onChange={onChange}
          />
        )
      }
    />
  );
}
