import type { Caller } from '../../bindings/Caller';
import type { CallLogView } from '../../bindings/CallLogView';

const digits = (text: string): string => text.replace(/\D/g, '');

/** What a dial sends: only `[0-9+*#]` (spaces, dashes and brackets go). */
export const dialable = (text: string): string => text.replace(/[^0-9+*#]/g, '');

/**
 * Whether `caller` matches what was typed: a name contains it (any case),
 * or a number contains its digits (spaces, dashes and `+` ignored). A query
 * with `+` may also drop a country code of up to 3 digits, so "+91 98765"
 * matches a number stored as "98765 43210".
 */
export function matchesCaller(caller: Caller, query: string): boolean {
  const q = query.trim();
  if (q === '') return true;
  if (caller.name?.toLocaleLowerCase().includes(q.toLocaleLowerCase())) return true;
  if (/\p{L}/u.test(q)) return false;
  const want = digits(q);
  const have = digits(caller.number);
  if (want === '') return false;
  if (have.includes(want)) return true;
  // Past the country code, at least 4 digits so it still narrows.
  return (
    q.startsWith('+') && [1, 2, 3].some((n) => want.length - n >= 4 && have.includes(want.slice(n)))
  );
}

/** `log` with only the calls matching `query`; days left empty go. */
export function filterLog(log: CallLogView, query: string): CallLogView {
  if (query.trim() === '') return log;
  const groups = log.groups
    .map((g) => ({ ...g, rows: g.rows.filter((r) => matchesCaller(r.caller, query)) }))
    .filter((g) => g.rows.length > 0);
  return { ...log, groups };
}
