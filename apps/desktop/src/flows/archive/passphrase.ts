import { ENGINE_DEFAULTS } from '../../bindings/constants';
import type { PassphraseStrength } from '../../bindings/PassphraseStrength';

const { thresholds, new_archive_minimum: minimum } = ENGINE_DEFAULTS.passphrase;

/** Strengths weakest first, as the engine's policy orders them. */
const ORDER: readonly PassphraseStrength[] = ['weak', ...thresholds.map((t) => t.strength)];

/**
 * Whether a passphrase of `strength` may protect a new archive (the engine's
 * `new_archive_minimum`; export refuses anything weaker).
 */
export function strongEnough(strength: PassphraseStrength): boolean {
  return ORDER.indexOf(strength) >= ORDER.indexOf(minimum);
}
