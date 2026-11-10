import type { BridgeError } from '../bindings/BridgeError';
import type { ErrorCode } from '../bindings/ErrorCode';

/**
 * Reject the current call with `error`. The port contract rejects with plain
 * `BridgeError` data (what the bridge serialises), not an `Error` instance;
 * this is the one place that throws it.
 */
export function raise(error: BridgeError): never {
  // eslint-disable-next-line @typescript-eslint/only-throw-error
  throw error;
}

/** A `BridgeError` of `code`. */
export function bridgeError(
  code: ErrorCode,
  message: string,
  hint: string | null = null,
): BridgeError {
  return { code, message, hint };
}
