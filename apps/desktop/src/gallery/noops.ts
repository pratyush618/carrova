/**
 * An intents object whose every handler does nothing. Listing the keys keeps
 * it type-checked against the screen's intents interface.
 */
export function noops<K extends string>(...keys: readonly K[]): Record<K, () => void> {
  const noop = (): void => undefined;
  // Every key in `keys` is present by construction, so the record is total.
  return Object.fromEntries(keys.map((k) => [k, noop])) as Record<K, () => void>;
}
