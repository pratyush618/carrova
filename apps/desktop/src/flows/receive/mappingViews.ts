import type { PathMapping } from '../../bindings/PathMapping';
import type { OsKind, PlanReview } from '../../engine/types';
import type { MappedPath, MappingModel, MappingRow } from '../../screens/receive/model';
import { appOf } from '../shared/keys';
import { appRef } from '../shared/paths';
import { itemLabel } from './reviewViews';

type Rooted = PathMapping['to'];

const pathOf = ([root, rel]: Rooted, abs: string | null): MappedPath => ({ root, rel, abs });

const samePlace = (m: PathMapping): boolean => m.from[0] === m.to[0] && m.from[1] === m.to[1];

/**
 * "Where things land": each mapped slot with its source path (semantic, the
 * source's directories are unknown here) and its real target path.
 */
export function mappingModel(review: PlanReview, here: OsKind): MappingModel | null {
  const from = review.source?.os;
  if (from === undefined) return null;
  const rows = review.plan.mappings.map((m, i): [MappingRow, boolean] => {
    const app = appOf(m.item) ?? m.item;
    return [
      {
        key: `${m.item}#${String(i)}`,
        glyph: appRef(app, app).glyph,
        title: itemLabel(review, m.item),
        from: pathOf(m.from, null),
        to: pathOf(m.to, m.to_abs),
      },
      samePlace(m),
    ];
  });
  return {
    from,
    to: here,
    moved: rows.filter(([, same]) => !same).map(([row]) => row),
    same: rows.filter(([, same]) => same).map(([row]) => row),
  };
}
