import type { ReactNode } from 'react';
import type { FeatureRowView, FeatureState } from '../../engine/types';
import { useTranslation } from 'react-i18next';
import { AppGlyph, Row, Segmented, Toggle } from '../../ui';
import { FEATURE_ICON, type Phase2Feature } from './model';

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
  body,
  waiting = false,
  action,
  onChange,
}: {
  readonly feature: Phase2Feature;
  readonly row: FeatureRowView;
  readonly device: string;
  /** The device is away: dimmed, still changeable (the choice is kept). */
  readonly off: boolean;
  /** What the feature does, where this side words it differently (the phone). */
  readonly body?: string;
  /** The phone lacks the access it needs: that replaces the description. */
  readonly waiting?: boolean;
  /** A small button beside the control (the phone's app list). */
  readonly action?: ReactNode;
  readonly onChange: (state: FeatureState) => void;
}) {
  const { t } = useTranslation();
  const title = t(`ecosystem.features.${feature}.title`);
  const toggle = (
    <Toggle
      on={row.local === 'on'}
      label={title}
      onChange={(on) => {
        onChange(on ? 'on' : 'off');
      }}
    />
  );
  return (
    <Row
      off={off}
      lead={<AppGlyph glyph={{ icon: FEATURE_ICON[feature] }} size="sm" />}
      title={title}
      sub={
        row.remote === 'off'
          ? t('ecosystem.offOn', { name: device })
          : waiting
            ? t('ecosystem.device.waiting')
            : (body ?? t(`ecosystem.features.${feature}.body`))
      }
      trail={
        row.group !== 'quiet' ? (
          <Segmented<FeatureState>
            label={title}
            options={(['ask', 'off'] as const).map((value) => ({
              value,
              label: t(`ecosystem.states.${value}`),
            }))}
            value={row.local}
            onChange={onChange}
          />
        ) : action === undefined ? (
          toggle
        ) : (
          <span className="s-device__ctl">
            {action}
            {toggle}
          </span>
        )
      }
    />
  );
}
