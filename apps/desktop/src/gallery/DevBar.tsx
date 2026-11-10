import { PSEUDO_LOCALE } from '../i18n/pseudo';
import { usePreferences } from '../preferences/preferences';
import { Button, Kbd } from '../ui';
import type { GalleryScreen } from './types';

export interface DevBarProps {
  readonly screen: GalleryScreen;
  readonly indexOpen: boolean;
  readonly onIndex: () => void;
  readonly onStep: (delta: 1 | -1) => void;
}

function Switch({
  on,
  onClick,
  children,
}: {
  readonly on: boolean;
  readonly onClick: () => void;
  readonly children: string;
}) {
  return (
    <Button aria-pressed={on} onClick={onClick}>
      {children}
    </Button>
  );
}

/** The mockup's dev bar: index, prev/next, dark, right-to-left, reduced motion, pseudo-locale. */
export function DevBar({ screen, indexOpen, onIndex, onStep }: DevBarProps) {
  const { prefs, update } = usePreferences();
  const dark = prefs.theme === 'dark';
  return (
    <header className="g-dev" aria-label="Gallery controls">
      <span className="g-dev__title">Carrova screens</span>
      <Button aria-expanded={indexOpen} onClick={onIndex}>
        Screen index <Kbd>i</Kbd>
      </Button>
      <span className="g-dev__where">
        <b>{screen.title}</b>
      </span>
      <Button
        aria-label="Previous screen"
        onClick={() => {
          onStep(-1);
        }}
      >
        <Kbd>[</Kbd> Prev
      </Button>
      <Button
        aria-label="Next screen"
        onClick={() => {
          onStep(1);
        }}
      >
        Next <Kbd>]</Kbd>
      </Button>
      <Switch
        on={dark}
        onClick={() => {
          update({ theme: dark ? 'light' : 'dark' });
        }}
      >
        Dark
      </Switch>
      <Switch
        on={prefs.dir === 'rtl'}
        onClick={() => {
          update({ dir: prefs.dir === 'rtl' ? 'ltr' : 'rtl' });
        }}
      >
        Right to left
      </Switch>
      <Switch
        on={prefs.reduceMotion}
        onClick={() => {
          update({ reduceMotion: !prefs.reduceMotion });
        }}
      >
        Reduce motion
      </Switch>
      <Switch
        on={prefs.locale === PSEUDO_LOCALE}
        onClick={() => {
          update({ locale: prefs.locale === PSEUDO_LOCALE ? 'en' : PSEUDO_LOCALE });
        }}
      >
        Pseudo-locale
      </Switch>
    </header>
  );
}
