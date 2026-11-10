/** Words autocomplete after this many letters (EFF short words have unique 3-letter prefixes). */
const MIN_PREFIX = 3;
/** A code is a number and two words. */
const CODE_PARTS = 3;

const lastPart = (value: string): string => value.split(/[-\s]/).pop() ?? '';

/** The unique word `value`'s last part is a prefix of, if it is long enough. */
export function suggestWord(value: string, words: readonly string[]): string | null {
  const part = lastPart(value).toLowerCase();
  if (part.length < MIN_PREFIX) return null;
  const match = words.find((w) => w.startsWith(part));
  return match !== undefined && match !== part ? match : null;
}

/** `value` with its last part completed (and the next dash added), or null if nothing to accept. */
export function acceptSuggestion(value: string, words: readonly string[]): string | null {
  const word = suggestWord(value, words);
  if (word === null) return null;
  const head = value.slice(0, value.length - lastPart(value).length);
  const parts = value.split('-').length;
  return head + word + (parts < CODE_PARTS ? '-' : '');
}
