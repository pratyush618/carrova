import { describe, expect, it } from 'vitest';
import { ENGINE_DEFAULTS } from '@/bindings/constants';
import { strongEnough } from '@/flows/archive/passphrase';

describe('the archive passphrase policy', () => {
  it("accepts the engine's new-archive minimum and anything stronger", () => {
    expect(ENGINE_DEFAULTS.passphrase.new_archive_minimum).toBe('fair');
    expect(strongEnough('weak')).toBe(false);
    expect(strongEnough('fair')).toBe(true);
    expect(strongEnough('strong')).toBe(true);
  });
});
