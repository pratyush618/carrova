import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { buildChrome } from '../app/shell/chrome';
import { Button, ChromeContext, type HubStatus, Window } from '../ui';
import { DevBar } from './DevBar';
import { ScreenIndex } from './ScreenIndex';
import { hashFor, neighbour, resolve } from './registry';
import { useHashTarget } from './useHashTarget';
import './gallery.css';

const ignore = (): void => undefined;

/** Fixture for the sidebar foot (the shell reads the real bundle state). */
const HUB_STATUS: HubStatus = {
  title: 'Recipes up to date',
  tone: 'success',
  detail: 'Bundle 2026.11.3 · signed',
};

/** Dev-only screen gallery: every screen and state from fixtures, like the mockup's dev bar. */
export function Gallery() {
  const { t } = useTranslation();
  const [target, go] = useHashTarget();
  const [indexOpen, setIndexOpen] = useState(false);
  const ref = resolve(target);
  const current = ref?.screen;

  useEffect(() => {
    if (!current) return undefined;
    const onKey = (event: KeyboardEvent) => {
      if (event.altKey || event.ctrlKey || event.metaKey) return;
      if (event.target instanceof HTMLElement && event.target.closest('input, select, textarea'))
        return;
      if (event.key === ']') go(hashFor(neighbour(current, 1), ''));
      if (event.key === '[') go(hashFor(neighbour(current, -1), ''));
      if (event.key === 'i') setIndexOpen((open) => !open);
    };
    document.addEventListener('keydown', onKey);
    return () => {
      document.removeEventListener('keydown', onKey);
    };
  }, [current, go]);

  if (!ref) return null;
  const { screen, state } = ref;
  const rendered = screen.render(state);
  const chrome = buildChrome(rendered.view ?? screen.view, t, ignore, HUB_STATUS);
  const frame = {
    'data-testid': 'gallery-frame',
    'data-screen': screen.id,
    'data-state': state,
    'data-states': screen.states.map((s) => s.id).join(' '),
  };

  return (
    <div className="g-root">
      <DevBar
        screen={screen}
        indexOpen={indexOpen}
        onIndex={() => {
          setIndexOpen((open) => !open);
        }}
        onStep={(delta) => {
          go(hashFor(neighbour(screen, delta), ''));
        }}
      />
      {indexOpen && (
        <ScreenIndex
          current={screen}
          onClose={() => {
            setIndexOpen(false);
          }}
        />
      )}
      <div className="g-stage">
        {screen.states.length > 0 && (
          <div className="g-states" role="group" aria-label="States of this screen">
            {screen.states.map((s) => (
              <Button
                key={s.id}
                size="sm"
                aria-current={s.id === state}
                onClick={() => {
                  go(hashFor(screen, s.id));
                }}
              >
                {s.label}
              </Button>
            ))}
          </div>
        )}
        {screen.doc ? (
          <div className="g-doc" {...frame}>
            {rendered.base}
          </div>
        ) : (
          <div className="g-frame" {...frame}>
            <ChromeContext value={chrome}>
              <Window key={`${screen.id}:${state}`} overlays={rendered.overlays ?? []}>
                {rendered.base}
              </Window>
            </ChromeContext>
          </div>
        )}
      </div>
    </div>
  );
}
