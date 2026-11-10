import { IconButton } from '../ui';
import { SCREENS, hashFor } from './registry';
import type { GalleryScreen } from './types';

const groups = [...new Set(SCREENS.map((s) => s.group))];

/** Every screen, grouped like the mockup's index. */
export function ScreenIndex({
  current,
  onClose,
}: {
  readonly current: GalleryScreen;
  readonly onClose: () => void;
}) {
  return (
    <nav className="g-index" aria-label="Screen index">
      <div className="g-index__head">
        <b>Screen index</b>
        <IconButton icon="close" label="Close index" onClick={onClose} />
      </div>
      {groups.map((group) => (
        <section key={group}>
          <h3>{group}</h3>
          <ul>
            {SCREENS.filter((s) => s.group === group).map((s) => (
              <li key={s.id}>
                <a
                  href={hashFor(s, s.states[0]?.id ?? '')}
                  aria-current={s === current ? 'page' : undefined}
                  onClick={onClose}
                >
                  {s.title}
                </a>
              </li>
            ))}
          </ul>
        </section>
      ))}
    </nav>
  );
}
