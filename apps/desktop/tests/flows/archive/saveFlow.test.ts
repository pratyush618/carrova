import { describe, expect, it } from 'vitest';
import type { PassphraseStrength } from '@/bindings/PassphraseStrength';
import {
  canSave,
  initialSaveFlow,
  saveFlowReducer,
  saveLockModel,
  type SaveFlowAction,
} from '@/flows/archive/saveFlow';

const run = (...actions: SaveFlowAction[]) =>
  actions.reduce(saveFlowReducer, initialSaveFlow('carrova.carrova'));

describe('save flow', () => {
  const typed = (value: string, strength: PassphraseStrength): SaveFlowAction[] => [
    { type: 'passphrase', value },
    { type: 'rated', passphrase: value, strength },
  ];

  it('refuses to save with a weak or unconfirmed passphrase', () => {
    expect(canSave(run(...typed('summer', 'weak'), { type: 'confirm', value: 'summer' }))).toBe(
      false,
    );
    const strong = 'orbit lemon tiger canvas';
    expect(canSave(run(...typed(strong, 'strong')))).toBe(false);
    expect(canSave(run(...typed(strong, 'strong'), { type: 'confirm', value: strong }))).toBe(true);
  });

  it('follows the engine policy minimum: fair is enough', () => {
    const fair = 'orbit lemon';
    expect(canSave(run(...typed(fair, 'fair'), { type: 'confirm', value: fair }))).toBe(true);
  });

  it('waits for the rating of what is typed now', () => {
    const late = run(
      { type: 'passphrase', value: 'orbit lemon tiger' },
      { type: 'rated', passphrase: 'orbit', strength: 'weak' },
      { type: 'confirm', value: 'orbit lemon tiger' },
    );
    expect(canSave(late)).toBe(false);
    expect(saveLockModel(late, false).strength).toBe('weak');
  });

  it('moves to done when the archive is written and shows a failure on the lock screen', () => {
    const done = run({ type: 'save', action: { type: 'ok', value: { chunks: 1, bytes: 10 } } });
    expect(done.step).toBe('done');
    const error = { code: 'internal', message: 'Disk gone', hint: null } as const;
    const failed = run(
      { type: 'step', step: 'lock' },
      { type: 'save', action: { type: 'fail', error } },
    );
    expect(failed.step).toBe('lock');
    expect(saveLockModel(failed, false).error).toEqual(error);
  });
});
