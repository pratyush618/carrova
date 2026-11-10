import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from 'react';
import { i18n, type Locale } from '../i18n/i18n';
import { ReduceMotionOverride } from '../ui';

/** `system` follows `prefers-color-scheme` live. */
export type ThemePreference = 'system' | 'light' | 'dark';
export type Direction = 'ltr' | 'rtl';

/** Presentation preferences applied to the document root. */
export interface Preferences {
  readonly theme: ThemePreference;
  /** On top of the OS setting, never instead of it. */
  readonly reduceMotion: boolean;
  readonly locale: Locale;
  readonly dir: Direction;
}

export const defaultPreferences: Preferences = {
  theme: 'system',
  reduceMotion: false,
  locale: 'en',
  dir: 'ltr',
};

interface PreferencesApi {
  readonly prefs: Preferences;
  readonly update: (patch: Partial<Preferences>) => void;
}

const PreferencesContext = createContext<PreferencesApi | null>(null);

/** Writes preferences onto `<html>`, where the token and motion stylesheets read them. */
export function applyToDocument(root: HTMLElement, prefs: Preferences): void {
  if (prefs.theme === 'system') delete root.dataset['theme'];
  else root.dataset['theme'] = prefs.theme;
  if (prefs.reduceMotion) root.dataset['motion'] = 'reduce';
  else delete root.dataset['motion'];
  root.lang = prefs.locale;
  root.dir = prefs.dir;
}

export function PreferencesProvider({
  initial = defaultPreferences,
  children,
}: {
  readonly initial?: Preferences;
  readonly children: ReactNode;
}) {
  const [prefs, setPrefs] = useState(initial);

  useEffect(() => {
    applyToDocument(document.documentElement, prefs);
    if (i18n.language !== prefs.locale) void i18n.changeLanguage(prefs.locale);
  }, [prefs]);

  // Stable, so effects that apply saved settings don't re-run on every change.
  const update = useCallback((patch: Partial<Preferences>) => {
    setPrefs((p) => ({ ...p, ...patch }));
  }, []);
  const api = useMemo<PreferencesApi>(() => ({ prefs, update }), [prefs, update]);

  return (
    <PreferencesContext value={api}>
      <ReduceMotionOverride value={prefs.reduceMotion}>{children}</ReduceMotionOverride>
    </PreferencesContext>
  );
}

/** Current preferences and a way to change them. */
export function usePreferences(): PreferencesApi {
  const api = useContext(PreferencesContext);
  if (api === null) throw new Error('usePreferences must be used inside PreferencesProvider');
  return api;
}
