import { describe, expect, it } from 'vitest';
import { arrivedWithout, movingAt, reviewWith, skippedApps } from '../../src/live/ReceiveScene';
import { reviewDecide } from '../../../desktop/src/screens/receive/fixtures';
import type { DecisionModel } from '../../../desktop/src/screens/receive/model';

const answered = (answers: Record<string, DecisionModel['answer']>): DecisionModel[] =>
  reviewDecide.decisions.map((d) => ({ ...d, answer: answers[d.key] ?? null }) as DecisionModel);

describe('reviewWith', () => {
  it('blocks approval until every question has an answer', () => {
    expect(reviewWith(answered({ 'ssh/keys': 'bring' })).blocked).toBe(true);
    const all = answered({ 'ssh/keys': 'bring', 'gpg/secret-keys': 'leave', vscode: 'closed' });
    expect(reviewWith(all).blocked).toBe(false);
  });

  it('lists only the keys the visitor chose to bring', () => {
    const all = answered({ 'ssh/keys': 'bring', 'gpg/secret-keys': 'leave', vscode: 'closed' });
    expect(reviewWith(all).rail.keys).toEqual(['Your SSH keys']);
    expect(reviewWith(answered({})).rail.keys).toBeNull();
  });
});

describe('movingAt', () => {
  const apps = (share: number) => {
    const m = movingAt(share);
    if (m.status !== 'transfer') throw new Error('expected a transfer');
    return m.transfer;
  };

  it('starts with nothing arrived', () => {
    const t = apps(0);
    expect(t.phase).toBe('moving');
    expect(t.done).toBe(0);
    expect(t.apps.every((a) => a.state === 'waiting')).toBe(true);
  });

  it('has exactly one app on its way mid-move', () => {
    expect(apps(0.5).apps.filter((a) => a.state === 'moving')).toHaveLength(1);
  });

  it('counts bytes and time left down as it goes, never to zero seconds mid-move', () => {
    const half = apps(0.5);
    expect(half.done).toBe(half.total / 2);
    expect(half.secondsLeft).toBe(60);
    expect(apps(0.999).secondsLeft).toBe(1);
  });

  it('moves into putting things in place once everything has arrived', () => {
    expect(apps(1).phase).toBe('applying');
  });
});

describe('a skipped app', () => {
  const skip = answered({ 'ssh/keys': 'bring', 'gpg/secret-keys': 'leave', vscode: 'skip' });
  const close = answered({ 'ssh/keys': 'bring', 'gpg/secret-keys': 'leave', vscode: 'closed' });

  it('is skipped only when the visitor says skip', () => {
    expect(skippedApps(skip)).toEqual(['VS Code']);
    expect(skippedApps(close)).toEqual([]);
  });

  it('never moves, and is left off the receipt', () => {
    const names = (share: number) => {
      const m = movingAt(share, ['VS Code']);
      if (m.status !== 'transfer') throw new Error('expected a transfer');
      return m.transfer.apps.map((a) => a.name);
    };
    expect(names(0.5)).not.toContain('VS Code');
    expect(names(1)).not.toContain('VS Code');
    const receipt = arrivedWithout(['VS Code']);
    expect(receipt.moved).not.toContain('VS Code');
    expect(receipt.toInstall).not.toContain('VS Code');
    expect(arrivedWithout([]).moved).toContain('VS Code');
  });
});
