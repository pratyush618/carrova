import { readFileSync } from 'node:fs';
import { resolve } from 'node:path';
import { describe, expect, it } from 'vitest';
import { PAIRING_WORDS } from '@/engine/wordlist/words';

/** Vitest runs from the desktop package; the engine source is two levels up. */
const RUST_SOURCE = resolve(process.cwd(), '../../crates/carrova-crypto/src/pairing/wordlist.rs');

/** The quoted words of the Rust `WORDS` array, in order. */
function rustWords(): string[] {
  const source = readFileSync(RUST_SOURCE, 'utf8');
  const array = source.slice(source.indexOf('= ['));
  return [...array.matchAll(/"([a-z]+)"/g)].map((m) => m[1] ?? '');
}

describe('pairing wordlist', () => {
  it('matches the engine wordlist word for word', () => {
    expect(PAIRING_WORDS).toEqual(rustWords());
  });

  it('has 1296 words with unique three-letter prefixes', () => {
    expect(PAIRING_WORDS).toHaveLength(1296);
    expect(new Set(PAIRING_WORDS.map((w) => w.slice(0, 3))).size).toBe(1296);
  });
});
