import { useTranslation } from 'react-i18next';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Stepper.css';

export interface StepperProps {
  /** Every step's label, in order ("Pick", "Pair", …). */
  readonly steps: readonly string[];
  /** 0-based index of the step the user is on. */
  readonly current: number;
}

/** Labelled steps, so the shape of the whole task is learnable at a glance. */
export function Stepper({ steps, current }: StepperProps) {
  const { t } = useTranslation();
  return (
    <ol className="p-steps" aria-label={t('flow.step', { step: current + 1, total: steps.length })}>
      {steps.map((label, i) => (
        <li
          key={label}
          className={cx(i < current && 'p-steps--done', i === current && 'p-steps--now')}
          aria-current={i === current ? 'step' : undefined}
        >
          <i aria-hidden="true">{i < current ? <Icon name="check" size="sm" /> : i + 1}</i>
          {label}
        </li>
      ))}
    </ol>
  );
}
