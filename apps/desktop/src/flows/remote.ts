import type { BridgeError } from '../engine/types';

/**
 * A value the engine provides on request: not asked yet, on its way, there,
 * or failed. While reloading (or after a failure) the last value stays
 * visible, so screens never flash empty.
 */
export type Remote<T> =
  | { readonly status: 'idle' }
  | { readonly status: 'loading'; readonly last: T | null }
  | { readonly status: 'ready'; readonly value: T }
  | { readonly status: 'failed'; readonly error: BridgeError; readonly last: T | null };

export type RemoteAction<T> =
  | { readonly type: 'start' }
  | { readonly type: 'ok'; readonly value: T }
  | { readonly type: 'fail'; readonly error: BridgeError }
  | { readonly type: 'reset' };

export const idle: Remote<never> = { status: 'idle' };

/** The freshest value a remote has, if any. */
export function current<T>(remote: Remote<T>): T | null {
  switch (remote.status) {
    case 'idle':
      return null;
    case 'ready':
      return remote.value;
    case 'loading':
    case 'failed':
      return remote.last;
  }
}

/** The remote reducer: one request's life. */
export function remoteReducer<T>(remote: Remote<T>, action: RemoteAction<T>): Remote<T> {
  switch (action.type) {
    case 'start':
      return { status: 'loading', last: current(remote) };
    case 'ok':
      return { status: 'ready', value: action.value };
    case 'fail':
      return { status: 'failed', error: action.error, last: current(remote) };
    case 'reset':
      return idle;
  }
}
