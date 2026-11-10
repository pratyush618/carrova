import i18next from 'i18next';
import { initReactI18next } from 'react-i18next';
import { en } from './en';
import { PSEUDO_LOCALE, pseudoLocalize } from './pseudo';

/** Locales the app ships. The pseudo-locale exists for layout and coverage testing. */
export const LOCALES = ['en', PSEUDO_LOCALE] as const;
export type Locale = (typeof LOCALES)[number];

/** The app's i18next instance, wired into react-i18next. */
export const i18n = i18next.createInstance();

void i18n.use(initReactI18next).init({
  lng: 'en',
  fallbackLng: 'en',
  supportedLngs: LOCALES,
  resources: {
    en: { translation: en },
    [PSEUDO_LOCALE]: { translation: pseudoLocalize(en) },
  },
  // React escapes output; double-escaping would show entities.
  interpolation: { escapeValue: false },
  returnNull: false,
  initAsync: false,
});
