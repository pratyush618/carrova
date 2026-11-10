import list from './words.json';

/**
 * The EFF short wordlist (CC BY 3.0, see `NOTICE`) the engine draws pairing
 * code words from, for autocomplete on the receiver. A copy of
 * `crates/carrova-crypto/src/pairing/wordlist.rs`; a test keeps them equal.
 */
export const PAIRING_WORDS: readonly string[] = list;
