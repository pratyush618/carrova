import type { en } from './en';

/** Type-safe `t()` keys and interpolations, checked against the English catalogue. */
declare module 'i18next' {
  interface CustomTypeOptions {
    defaultNS: 'translation';
    resources: { translation: typeof en };
    returnNull: false;
  }
}
