import { useTranslation } from 'react-i18next';
import type { OsKind } from '../../bindings/OsKind';
import { Badge } from '../Badge/Badge';
import { Icon } from '../Icon/Icon';
import { OsBadge } from './OsBadge';

/** One computer in a move. */
export interface RouteEnd {
  readonly os: OsKind;
  /** The computer's name ("ezio-thinkpad"). */
  readonly name: string;
}

export interface RouteProps {
  readonly from: RouteEnd;
  /** `null` until the other computer is paired. */
  readonly to: RouteEnd | null;
}

/** The flow header's route chip: this computer → the other one. */
export function Route({ from, to }: RouteProps) {
  const { t } = useTranslation();
  return (
    <span className="p-route">
      <OsBadge os={from.os} label={from.name} size="sm" />
      <Icon name="arrow" size="sm" />
      <span className="p-vh">{t('ui.route.to')}</span>
      {to ? (
        <OsBadge os={to.os} label={to.name} size="sm" />
      ) : (
        <Badge>{t('ui.route.notPaired')}</Badge>
      )}
    </span>
  );
}
