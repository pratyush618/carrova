import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './AppGlyph.css';

export interface AppGlyphProps {
  /** Two or three characters from the recipe (`git`, `>_`, `{ }`). */
  readonly glyph: string;
  readonly size?: 'md' | 'sm';
  /** Replaced by a tick that springs in. */
  readonly done?: boolean;
}

/** The app's monogram tile. Decorative: the app name is always next to it. */
export function AppGlyph({ glyph, size = 'md', done = false }: AppGlyphProps) {
  return (
    <span
      className={cx('p-app', size === 'sm' && 'p-app--sm', done && 'p-app--done')}
      aria-hidden="true"
    >
      {done ? <Icon name="check" /> : glyph}
    </span>
  );
}
