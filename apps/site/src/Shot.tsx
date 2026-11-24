import { useScheme } from './theme';

// `<name>.<scheme>-<width>.webp`, written by `just site-shots`.
const FILES = import.meta.glob<string>('./assets/shots/*.webp', { eager: true, import: 'default' });

/** Natural size (CSS px at 1x) of each capture. */
const SIZE = {
  review: [1100, 720],
  decide: [1100, 720],
  send: [1100, 720],
  code: [1100, 720],
  linked: [1100, 720],
  done: [1100, 720],
  nearby: [1100, 720],
  undo: [1100, 720],
  apps: [1100, 720],
  device: [888, 720],
  messages: [888, 720],
  note: [380, 200],
} as const;

export type ShotName = keyof typeof SIZE;

interface Variant {
  readonly width: number;
  readonly url: string;
}

const VARIANTS = new Map<string, Variant[]>();
for (const [path, url] of Object.entries(FILES)) {
  const m = /\/([a-z]+\.(?:light|dark))-(\d+)\.webp$/.exec(path);
  if (!m?.[1] || !m[2]) continue;
  const list = VARIANTS.get(m[1]) ?? [];
  list.push({ width: Number(m[2]), url });
  VARIANTS.set(
    m[1],
    list.sort((a, b) => a.width - b.width),
  );
}

export function shotVariants(name: ShotName, scheme: 'light' | 'dark'): readonly Variant[] {
  return VARIANTS.get(`${name}.${scheme}`) ?? [];
}

interface ShotProps {
  readonly name: ShotName;
  readonly alt: string;
  /** Rendered width hint for `srcset`. */
  readonly sizes: string;
  /** Above the fold: load now, at high priority. */
  readonly eager?: boolean;
  readonly className?: string | undefined;
}

/** A real app screenshot in the visitor's theme. */
export function Shot({ name, alt, sizes, eager = false, className }: ShotProps) {
  const variants = shotVariants(name, useScheme());
  const [w, h] = SIZE[name];
  return (
    <img
      className={className}
      src={variants.at(-1)?.url}
      srcSet={variants.map((v) => `${v.url} ${String(v.width)}w`).join(', ')}
      sizes={sizes}
      width={w}
      height={h}
      alt={alt}
      loading={eager ? 'eager' : 'lazy'}
      fetchPriority={eager ? 'high' : 'auto'}
      decoding="async"
    />
  );
}
