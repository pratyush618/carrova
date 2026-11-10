import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Manifest, Route, type ManifestEnd, type ManifestFact } from '../../ui';
import type { Computer, MoveRoute } from './model';

/** A known OS as the badge shows it ("Linux"). */
function useOsLabel(): (os: NonNullable<Computer['os']>) => string {
  const { t } = useTranslation();
  return (os) => t(`settings.about.os.${os}`);
}

/** How screens name a computer: its name, else "A Linux computer", else a neutral word. */
export function useComputerName(): (computer: Computer, here: boolean) => string {
  const { t } = useTranslation();
  const osLabel = useOsLabel();
  return (computer, here) => {
    if (computer.name !== null) return computer.name;
    if (here) return t('transfer.move.here');
    return computer.os === null
      ? t('transfer.move.other')
      : t('transfer.move.otherOs', { os: osLabel(computer.os) });
  };
}

function useEnd(): (computer: Computer, note: string, here: boolean) => ManifestEnd {
  const name = useComputerName();
  const osLabel = useOsLabel();
  return (computer, note, here) => ({
    name: name(computer, here),
    note,
    ...(computer.os !== null && { os: { kind: computer.os, label: osLabel(computer.os) } }),
  });
}

export interface MoveRailProps {
  readonly route: MoveRoute;
  /** Which end is this computer (the other end may be unnamed). */
  readonly here: 'from' | 'to';
  /** What each end is doing now ("Sending", "This computer"). */
  readonly notes: { readonly from: string; readonly to: string };
  readonly facts: readonly ManifestFact[];
  readonly promise: ReactNode;
  readonly live?: boolean;
}

/** The flow's "This move" rail for a known route. */
export function MoveRail({ route, here, notes, facts, promise, live = false }: MoveRailProps) {
  const end = useEnd();
  return (
    <Manifest
      from={end(route.from, notes.from, here === 'from')}
      to={end(route.to, notes.to, here === 'to')}
      facts={facts}
      promise={promise}
      live={live}
    />
  );
}

/** The header's route chip, once both computers' systems are known. */
export function RouteChip({ route, here }: Pick<MoveRailProps, 'route' | 'here'>) {
  const name = useComputerName();
  const { from, to } = route;
  if (from.os === null || to.os === null) return null;
  return (
    <Route
      from={{ os: from.os, name: name(from, here === 'from') }}
      to={{ os: to.os, name: name(to, here === 'to') }}
    />
  );
}
