/**
 * Runtime checks at the IPC boundary: what arrives from the bridge is
 * `unknown` until proven to have the contract's shape. The deep payload
 * types are generated from the same Rust types the bridge serialises, so
 * these guards check the discriminants and the fields UI logic branches on.
 */
import type { BridgeError } from '../bindings/BridgeError';
import type { BridgeEvent } from '../bindings/BridgeEvent';
import type { ErrorCode } from '../bindings/ErrorCode';

/** Every `ErrorCode`: a record, so the compiler insists on each new code. */
const ERROR_CODES: Readonly<Record<ErrorCode, true>> = {
  cancelled: true,
  peer_cancelled: true,
  wrong_passphrase: true,
  bad_code: true,
  pairing_failed: true,
  cannot_connect: true,
  invalid_selection: true,
  blocked: true,
  no_space: true,
  network: true,
  rolled_back: true,
  rollback_failed: true,
  session: true,
  recipes: true,
  internal: true,
};

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function isErrorCode(value: unknown): value is ErrorCode {
  return typeof value === 'string' && Object.hasOwn(ERROR_CODES, value);
}

/** Whether `value` is a `BridgeError` (`{ code, message, hint }`). */
export function isBridgeError(value: unknown): value is BridgeError {
  return (
    isRecord(value) &&
    isErrorCode(value['code']) &&
    typeof value['message'] === 'string' &&
    (value['hint'] === null || typeof value['hint'] === 'string')
  );
}

/** Whether `value` is a `BridgeEvent` of a known `kind`. */
export function isBridgeEvent(value: unknown): value is BridgeEvent {
  if (!isRecord(value)) {
    return false;
  }
  if (value['kind'] === 'send_requested') {
    return true;
  }
  if (typeof value['session'] !== 'string') {
    return false;
  }
  switch (value['kind']) {
    case 'engine':
      return isRecord(value['event']) && typeof value['event']['event'] === 'string';
    case 'pairing_code':
      return isRecord(value['code']) && typeof value['code']['code'] === 'string';
    case 'confirm_new_code':
      return typeof value['failures'] === 'number' && typeof value['remaining'] === 'number';
    case 'task_ended':
      return value['error'] === null || isBridgeError(value['error']);
    default:
      return false;
  }
}

/**
 * Any rejection as a `BridgeError`: the bridge rejects with one; anything
 * else (a Tauri or JS failure) becomes `internal` with its text.
 */
export function toBridgeError(reason: unknown): BridgeError {
  if (isBridgeError(reason)) {
    return reason;
  }
  const message = reason instanceof Error ? reason.message : String(reason);
  return { code: 'internal', message, hint: null };
}
