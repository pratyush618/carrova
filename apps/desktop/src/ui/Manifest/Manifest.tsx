import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { OsKind } from '../../bindings/OsKind';
import { LinkButton } from '../Button/Button';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import { OsBadge } from '../OsBadge/OsBadge';
import './Manifest.css';

/** One end of the move. */
export interface ManifestEnd {
  /** The computer's name, or a placeholder ("The other computer"). */
  readonly name: string;
  /** Its role or state right now ("This computer", "Paired · reviewing"). */
  readonly note: string;
  /** Absent until the computer is known: the end is drawn pending. */
  readonly os?: { readonly kind: OsKind; readonly label: string };
}

/** One line of the move's facts ("Keys and secrets — Left behind — Change"). */
export interface ManifestFact {
  readonly label: string;
  readonly value: ReactNode;
  /** Shows a "Change" link that jumps to where this is decided. */
  readonly onChange?: () => void;
}

export interface ManifestProps {
  readonly from: ManifestEnd;
  readonly to: ManifestEnd;
  /** Data is flowing: the route line turns solid accent. */
  readonly live?: boolean;
  readonly facts: readonly ManifestFact[];
  /** The safety promise that fits this moment. */
  readonly promise: ReactNode;
}

function End({ end, to }: { readonly end: ManifestEnd; readonly to: boolean }) {
  return (
    <>
      <span
        className={cx(
          'p-route__dot',
          to && 'p-route__dot--to',
          to && end.os && 'p-route__dot--set',
        )}
      />
      <div className="p-route__end">
        <b className={cx(!end.os && 'p-route__pending')}>{end.name}</b>{' '}
        {end.os && <OsBadge os={end.os.kind} label={end.os.label} size="sm" />}
        <div className="p-route__note">{end.note}</div>
      </div>
    </>
  );
}

/** The flow's rail: one object that grows through the flow (route, facts, promise). */
export function Manifest({ from, to, live = false, facts, promise }: ManifestProps) {
  const { t } = useTranslation();
  return (
    <div className="p-manifest">
      <div>
        <h2 className="p-manifest__title">
          <Icon name="box" />
          {t('ui.manifest.title')}
        </h2>
        <div className="p-route-v">
          <End end={from} to={false} />
          <span className={cx('p-route__line', live && 'p-route__line--live')} />
          <span />
          <End end={to} to />
        </div>
      </div>
      <dl className="p-facts">
        {facts.map((fact) => (
          <div key={fact.label} className="p-fact">
            <dt>{fact.label}</dt>
            <dd>
              {fact.value}
              {fact.onChange && (
                <LinkButton
                  aria-label={t('ui.manifest.change', { fact: fact.label })}
                  onClick={fact.onChange}
                >
                  {t('action.change')}
                </LinkButton>
              )}
            </dd>
          </div>
        ))}
      </dl>
      <p className="p-promise">
        <Icon name="shield" />
        <span>{promise}</span>
      </p>
    </div>
  );
}
