import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import type { OsKind } from '../../bindings/OsKind';
import { AppGlyph } from '../AppGlyph/AppGlyph';
import { OsBadge } from '../OsBadge/OsBadge';
import { splitChange, type PathSplit } from './diff';
import './MapRow.css';

/** Where an item sits on one computer. */
export interface MapEnd {
  readonly os: OsKind;
  /** Short OS name for the badge ("Ubuntu"). */
  readonly label: string;
  readonly path: string;
}

export interface MapRowProps {
  readonly from: MapEnd;
  readonly to: MapEnd;
  /** The app's monogram, when rows of several apps are listed together. */
  readonly glyph?: string;
  readonly title?: ReactNode;
  /** A chip after the title ("Check after"). */
  readonly tag?: ReactNode;
  /** One line under the paths (what to check after the move). */
  readonly note?: ReactNode;
}

function PathLine({
  end,
  split,
  kind,
  same,
}: {
  readonly end: MapEnd;
  readonly split: PathSplit;
  readonly kind: 'from' | 'to';
  readonly same?: string | undefined;
}) {
  return (
    <div className={`p-path p-path--${kind}`} dir="ltr">
      <OsBadge os={end.os} label={end.label} size="sm" />
      <code>
        {split.before}
        {split.changed && <mark>{split.changed}</mark>}
        {split.after}
      </code>
      {same && <span className="p-path__same">{same}</span>}
    </div>
  );
}

/** One cross-OS mapping: source path above target path, the changed segment highlighted. */
export function MapRow({ from, to, glyph, title, tag, note }: MapRowProps) {
  const { t } = useTranslation();
  const [a, b] = splitChange(from.path, to.path);
  const same = a.changed === '' && b.changed === '';
  return (
    <div className="p-maprow">
      {glyph !== undefined && <AppGlyph glyph={glyph} />}
      <div className="p-maprow__paths">
        {title !== undefined && (
          <p className="p-maprow__title">
            {title}
            {tag}
          </p>
        )}
        <PathLine end={from} split={a} kind="from" />
        <PathLine end={to} split={b} kind="to" same={same ? t('ui.mapping.same') : undefined} />
        {note !== undefined && <p className="p-maprow__note">{note}</p>}
      </div>
    </div>
  );
}
