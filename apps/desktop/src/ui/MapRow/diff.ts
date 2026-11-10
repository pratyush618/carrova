/** A path cut into the part that changes and the parts around it. */
export interface PathSplit {
  readonly before: string;
  readonly changed: string;
  readonly after: string;
}

const join = (segments: readonly string[]): string => segments.join('/');

function cut(segments: readonly string[], head: number, tail: number): PathSplit {
  let before = join(segments.slice(0, head));
  const changed = join(segments.slice(head, segments.length - tail));
  let after = join(segments.slice(segments.length - tail));
  // Separators stay in the unhighlighted parts, so `before + changed + after` is the path.
  if (before && (changed || after)) before += '/';
  if (after && changed) after = `/${after}`;
  return { before, changed, after };
}

/**
 * Splits two paths at `/` boundaries around the segments that differ, so the
 * mapping shows exactly what moves (`~/.config/Code` → `~/Library/…/Code`).
 * Identical paths come back with nothing changed.
 */
export function splitChange(from: string, to: string): readonly [PathSplit, PathSplit] {
  const a = from.split('/');
  const b = to.split('/');
  const shorter = Math.min(a.length, b.length);
  let head = 0;
  while (head < shorter && a[head] === b[head]) head++;
  let tail = 0;
  while (tail < shorter - head && a[a.length - 1 - tail] === b[b.length - 1 - tail]) tail++;
  return [cut(a, head, tail), cut(b, head, tail)];
}
