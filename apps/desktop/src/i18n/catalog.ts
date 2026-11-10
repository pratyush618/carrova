/** A message catalogue: nested sections of strings. */
export interface Catalog {
  readonly [key: string]: string | Catalog;
}

/** Every leaf of `catalog` as `[dotted.key, message]`. */
export function flatten(catalog: Catalog, prefix = ''): [string, string][] {
  return Object.entries(catalog).flatMap(([key, value]) => {
    const path = prefix ? `${prefix}.${key}` : key;
    return typeof value === 'string' ? [[path, value]] : flatten(value, path);
  });
}

/** `catalog` with every message passed through `fn`, same shape. */
export function mapMessages(catalog: Catalog, fn: (message: string) => string): Catalog {
  return Object.fromEntries(
    Object.entries(catalog).map(([key, value]) => [
      key,
      typeof value === 'string' ? fn(value) : mapMessages(value, fn),
    ]),
  );
}
