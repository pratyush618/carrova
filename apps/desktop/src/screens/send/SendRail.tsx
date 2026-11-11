import type { TFunction } from 'i18next';
import { useTranslation } from 'react-i18next';
import type { OsKind } from '../../bindings/OsKind';
import { useFormat } from '../../i18n/useFormat';
import { Manifest, Route, type ManifestEnd, type RouteEnd } from '../../ui';
import type { FoundPurpose, SendMove } from './model';

/** An OS family's name ("macOS"). */
export const osName = (os: OsKind, t: TFunction): string => t(`settings.about.os.${os}`);

/** A known computer as one end of the manifest. */
export const manifestEnd = (end: RouteEnd, note: string, t: TFunction): ManifestEnd => ({
  name: end.name,
  note,
  os: { kind: end.os, label: osName(end.os, t) },
});

export interface SendRailProps {
  readonly move: SendMove;
  readonly purpose: FoundPurpose;
  /** The other end once known (paired); a placeholder before. */
  readonly to?: ManifestEnd;
  /** Change a fact: opens the full selection. Absent once the selection is fixed. */
  readonly onChange?: () => void;
}

/** The send flow's "This move" rail: from here, to the other computer (or a file), and what moves. */
export function SendRail({ move, purpose, to, onChange }: SendRailProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const { facts } = move;
  const change = onChange === undefined ? {} : { onChange };
  return (
    <Manifest
      from={
        move.here === null
          ? { name: t('send.live.here'), note: t('send.rail.here') }
          : manifestEnd(move.here, t('send.rail.here'), t)
      }
      to={
        to ??
        (purpose === 'save'
          ? { name: t('send.rail.file'), note: t('send.rail.fileNext') }
          : { name: t('send.rail.other'), note: t('send.rail.pairNext') })
      }
      facts={
        facts === null
          ? []
          : [
              facts.picks === undefined
                ? {
                    label: t('send.rail.apps'),
                    value: t('summary.ofTotal', { done: facts.apps, total: facts.totalApps }),
                  }
                : {
                    label: t('send.manual.picked'),
                    value: t('send.manual.items', { count: facts.picks }),
                  },
              { label: t('send.rail.size'), value: format.bytes(facts.bytes) },
              {
                label: t('send.rail.keys'),
                value:
                  facts.keys === 0
                    ? t('send.rail.keysLeft')
                    : t('send.rail.keysBrought', { count: facts.keys }),
                ...change,
              },
              ...(facts.picks === undefined
                ? [
                    {
                      label: t('send.rail.caches'),
                      value: facts.caches ? t('send.rail.included') : t('send.rail.skipped'),
                      ...change,
                    },
                  ]
                : []),
              {
                label: t('send.rail.travels'),
                value: purpose === 'save' ? t('send.rail.inFile') : t('send.rail.onNetwork'),
              },
            ]
      }
      promise={purpose === 'save' ? t('send.rail.promiseFile') : t('send.rail.promise')}
    />
  );
}

/** The header's route chip: this computer → the other one (or "Not paired yet"). */
export function RouteChip({
  here,
  to = null,
}: {
  readonly here: RouteEnd | null;
  readonly to?: RouteEnd | null;
}) {
  return here === null ? null : <Route from={here} to={to} />;
}
