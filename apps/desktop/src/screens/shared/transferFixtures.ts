import { APP_ORDER, GB, MB, RECEIVER, SENDER, TOTAL_BYTES } from './fixtures';
import type { TransferApp, TransferModel, TransferPhase } from './transfer';

type AppState = TransferApp['state'];

const apps = (states: readonly AppState[], movingDone = 0): TransferApp[] =>
  APP_ORDER.map((app, i) => {
    const state = states[i] ?? 'waiting';
    return { ...app, state, bytesDone: state === 'moving' ? movingDone : 0 };
  });

const PHASE_APPS: Record<TransferPhase, TransferApp[]> = {
  moving: apps(['moving', 'waiting', 'waiting', 'waiting', 'waiting']),
  paused: apps(['done', 'done', 'done', 'done', 'waiting']),
  held: apps(['done', 'done', 'moving', 'waiting', 'waiting'], 40 * MB),
  reconnecting: apps(['done', 'done', 'done', 'done', 'waiting']),
  applying: apps(['done', 'done', 'done', 'moving', 'moving']),
};

const PHASE_DONE: Record<TransferPhase, number> = {
  moving: 0,
  paused: 612 * MB,
  held: 300 * MB,
  reconnecting: 612 * MB,
  applying: TOTAL_BYTES,
};

/** A transfer in `phase`, seen from `side` (mockup numbers: 1.2 GB, 612 MB in). */
export function transferAt(phase: TransferPhase, side: TransferModel['side']): TransferModel {
  return {
    side,
    phase,
    peer: side === 'send' ? RECEIVER : SENDER,
    done: PHASE_DONE[phase],
    total: TOTAL_BYTES,
    secondsLeft: phase === 'moving' ? 120 : null,
    bytesPerSecond: phase === 'moving' ? 10 * MB : null,
    route: {
      from: { name: SENDER, os: 'linux' },
      to: { name: RECEIVER, os: 'linux' },
    },
    apps: PHASE_APPS[phase],
    shortfall: 0.4 * GB,
  };
}

/** Mid-transfer with the big app moving (the design-system specimen). */
export const transferMoving: TransferModel = {
  ...transferAt('moving', 'receive'),
  done: 720 * MB,
  secondsLeft: 60,
  apps: apps(['done', 'done', 'done', 'done', 'moving'], 612 * MB),
};
