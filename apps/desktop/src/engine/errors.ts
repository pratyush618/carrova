import type { TFunction } from 'i18next';
import type { BridgeError } from './types';

/** Every rejected port call as a `BridgeError` (the IPC layer's one guard). */
export { toBridgeError } from '../ipc/guards';

/** What the UI says about a failure: a title from the catalogue, the engine's words below it. */
export interface ErrorText {
  readonly title: string;
  readonly body: string;
}

/** The one place a `BridgeError.code` becomes words. */
export function errorText(error: BridgeError, t: TFunction): ErrorText {
  const body = error.hint === null ? error.message : `${error.message} ${error.hint}`;
  return { title: t(`errors.${error.code}`), body };
}
