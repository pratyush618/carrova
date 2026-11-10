import { useTranslation } from 'react-i18next';
import type { UndoCounts } from './model';
import './undo.css';

/**
 * Exact counts, not vibes: what undo removes and puts back. The dialog adds
 * what it keeps of the user's later edits; the History rail (`brief`) says
 * everything else is left alone.
 */
export function UndoCalc({
  counts,
  brief = false,
}: {
  readonly counts: UndoCounts;
  readonly brief?: boolean;
}) {
  const { t } = useTranslation();
  const lines = [
    [t('undo.confirm.removes'), t('undo.confirm.newFiles', { count: counts.removed })],
    [t('undo.confirm.putsBack'), t('undo.confirm.yourFiles', { count: counts.restored })],
    brief
      ? [t('undo.confirm.leavesAlone'), t('undo.confirm.everythingElse')]
      : [
          t('undo.confirm.changedSince'),
          counts.movedAside === 0
            ? t('undo.confirm.none')
            : t('undo.confirm.keptAside', { count: counts.movedAside }),
        ],
  ] as const;
  return (
    <dl className="s-undo-calc">
      {lines.map(([what, count]) => (
        <div key={what}>
          <dt>{what}</dt>
          <dd>{count}</dd>
        </div>
      ))}
    </dl>
  );
}
