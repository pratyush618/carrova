import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import './AppGlyph.css';

export interface AppGlyphProps {
  /** Two or three characters from the recipe (`git`, `>_`, `{ }`), or an icon. */
  readonly glyph: string | { readonly icon: IconName };
  readonly size?: 'md' | 'sm';
  /** `warning` = sensitive (keys), `accent` = plugin, `success` = arrived. */
  readonly tone?: 'neutral' | 'warning' | 'accent' | 'success';
  /** Replaced by a tick that springs in (success tone). */
  readonly done?: boolean;
}

/** The app's monogram tile. Decorative: the app name is always next to it. */
export function AppGlyph({ glyph, size = 'md', tone = 'neutral', done = false }: AppGlyphProps) {
  const shown = done ? 'success' : tone;
  return (
    <span
      className={cx(
        'p-app',
        size === 'sm' && 'p-app--sm',
        shown !== 'neutral' && `p-app--${shown}`,
        done && 'p-app--done',
      )}
      aria-hidden="true"
    >
      {done ? (
        <Icon name="check" size="sm" />
      ) : typeof glyph === 'string' ? (
        glyph
      ) : (
        <Icon name={glyph.icon} size="sm" />
      )}
    </span>
  );
}
