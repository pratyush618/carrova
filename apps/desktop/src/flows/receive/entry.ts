import type { BridgeError } from '../../engine/types';

/** Typing the sender's code (LAN). */
export interface CodeEntry {
  readonly value: string;
  /** The `code@ip:port` fallback, offered once discovery failed. */
  readonly address: string;
  readonly status: 'typing' | 'connecting' | 'not-found' | 'mismatch' | 'failed';
  readonly error: BridgeError | null;
}

/** Choosing an archive file and its passphrase. */
export interface FileEntry {
  readonly path: string;
  readonly passphrase: string;
  readonly status: 'typing' | 'opening' | 'wrong' | 'failed';
  readonly error: BridgeError | null;
}

export const initialCode: CodeEntry = { value: '', address: '', status: 'typing', error: null };
export const initialFile: FileEntry = { path: '', passphrase: '', status: 'typing', error: null };

/**
 * How a failed `connect` reads: a wrong code (pairing failed), nobody found
 * (can't connect), or something else — a malformed code included.
 */
export function codeFailure(entry: CodeEntry, error: BridgeError): CodeEntry {
  switch (error.code) {
    case 'pairing_failed':
      // The sender burned the code; the old one can't work again.
      return { ...entry, value: '', status: 'mismatch', error };
    case 'cannot_connect':
    case 'network':
      return { ...entry, status: 'not-found', error };
    default:
      return { ...entry, status: 'failed', error };
  }
}

/** How a failed `openArchive` reads: the wrong passphrase, or something else. */
export function fileFailure(entry: FileEntry, error: BridgeError): FileEntry {
  return error.code === 'wrong_passphrase'
    ? { ...entry, status: 'wrong', error: null }
    : { ...entry, status: 'failed', error };
}

/** What `connect` gets: the typed address once offered and filled, else the code. */
export function connectTarget(entry: CodeEntry): string {
  const address = entry.address.trim();
  return entry.status === 'not-found' && address !== '' ? address : entry.value.trim();
}
