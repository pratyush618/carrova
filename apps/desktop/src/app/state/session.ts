import type {
  BridgeError,
  Checklist,
  CodeView,
  EngineEvent,
  Inventory,
  PauseReason,
  Phase,
  PlanSummary,
  RollbackOutcome,
  SessionId,
  SessionState,
} from '../../engine/types';

/** The LAN link as the UI shows it. */
export type LinkStatus = 'none' | 'connected' | 'retrying' | 'lost';

/** The sender's "a wrong code was tried, make a new one?" question. */
export interface NewCodePrompt {
  readonly failures: number;
  readonly remaining: number;
}

/** Everything the UI knows about one engine session, folded from its events. */
export interface SessionProgress {
  readonly id: SessionId;
  readonly state: SessionState | null;
  readonly phase: Phase | null;
  readonly done: number;
  readonly total: number;
  /** Bytes per item, in capture order (small configs first). */
  readonly captured: readonly { readonly item: string; readonly bytes: number }[];
  readonly link: LinkStatus;
  /** The other computer's address, once connected. */
  readonly peer: string | null;
  /** Sender: the receiver's device name, if it announced one (sanitised by the engine). */
  readonly peerName: string | null;
  /** Sender: what the receiver has. */
  readonly peerInventory: Inventory | null;
  /** Sender: the receiver's plan, while they review it. */
  readonly peerPlan: PlanSummary | null;
  readonly paused: PauseReason | null;
  readonly pairingFailures: number;
  /** Sender: the code to show (held in memory only, never logged). */
  readonly code: CodeView | null;
  readonly newCodePrompt: NewCodePrompt | null;
  readonly warnings: readonly string[];
  readonly rollback: Readonly<Record<RollbackOutcome, number>>;
  readonly checklist: Checklist | null;
  /** The session's background task ended (`error` null on success). */
  readonly ended: { readonly error: BridgeError | null } | null;
}

const NO_ROLLBACK: SessionProgress['rollback'] = {
  restored: 0,
  removed: 0,
  moved_aside: 0,
  failed: 0,
};

/** A session the UI has not heard from yet. */
/** How screens name the other computer: its announced name, else its address. */
export const peerLabel = (session: SessionProgress): string | null =>
  session.peerName ?? session.peer;

export function emptySession(id: SessionId): SessionProgress {
  return {
    id,
    state: null,
    phase: null,
    done: 0,
    total: 0,
    captured: [],
    link: 'none',
    peer: null,
    peerInventory: null,
    peerName: null,
    peerPlan: null,
    paused: null,
    pairingFailures: 0,
    code: null,
    newCodePrompt: null,
    warnings: [],
    rollback: NO_ROLLBACK,
    checklist: null,
    ended: null,
  };
}

/** Folds one engine event into a session. Pure; unknown facts are left alone. */
export function applyEvent(session: SessionProgress, event: EngineEvent): SessionProgress {
  switch (event.event) {
    case 'state_changed':
      return { ...session, state: event.to };
    case 'progress':
      return { ...session, phase: event.phase, done: event.done, total: event.total };
    case 'item_captured':
      return {
        ...session,
        captured: [...session.captured, { item: event.item, bytes: event.bytes }],
      };
    case 'rollback_step':
      return {
        ...session,
        rollback: { ...session.rollback, [event.outcome]: session.rollback[event.outcome] + 1 },
      };
    case 'warning':
      return { ...session, warnings: [...session.warnings, event.message] };
    case 'finished':
      return { ...session, checklist: event.checklist };
    case 'peer_connected':
      return { ...session, link: 'connected', peer: event.peer };
    case 'peer_lost':
      return { ...session, link: event.retrying ? 'retrying' : 'lost' };
    case 'inventory_received':
      return {
        ...session,
        peerName: event.device_name ?? session.peerName,
        peerInventory: {
          os: event.os,
          carrova_version: event.carrova_version,
          apps: event.apps,
          free_space: event.free_space,
          device_name: event.device_name ?? null,
        },
      };
    case 'plan_summary_received':
      return { ...session, peerPlan: { items: event.items } };
    case 'transfer_paused':
      return { ...session, paused: event.reason };
    case 'transfer_resumed':
      return { ...session, paused: null };
    case 'pairing_failed':
      return { ...session, pairingFailures: session.pairingFailures + 1 };
    case 'recaptured':
    case 'conflict_found':
    case 'action_applied':
      return session;
  }
}
