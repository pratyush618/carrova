import type { AskId, AskKind, ClipSkip, EngineEvent, TransferState } from '../../engine/types';

/** The bridge's session for linked-device events (`carrova_view::ipc::LINK_SESSION`). */
export const LINK_SESSION = 'link';

/** A device's offer waiting for the user's answer. */
export interface PendingAsk {
  readonly id: AskId;
  /** The offering device's id. */
  readonly device: string;
  readonly kind: AskKind;
}

/** One file moving to or from a linked device (as `carrova_view::ecosystem::Transfers` folds it). */
export interface Transfer {
  readonly device: string;
  readonly id: number;
  /** Empty until the file is saved: the progress events don't carry it. */
  readonly file_name: string;
  readonly done: number;
  readonly total: number;
  readonly state: TransferState;
}

/** What linked devices did that the UI shows outside any page. */
export interface LinkState {
  readonly asks: readonly PendingAsk[];
  /** Oldest first. */
  readonly transfers: readonly Transfer[];
  /** The clipboard this computer last kept to itself, and why. */
  readonly clip: { readonly reason: ClipSkip; readonly n: number } | null;
  /** Bumped when a link or its features change: the device list is stale. */
  readonly changes: number;
  /** Bumped when a clip arrives: the history is stale. */
  readonly clips: number;
}

export const emptyLink: LinkState = { asks: [], transfers: [], clip: null, changes: 0, clips: 0 };

/** Things the user did to the link state (answered, dismissed). */
export type LinkAction =
  | { readonly type: 'ask_answered'; readonly id: AskId }
  | { readonly type: 'transfer_dismissed'; readonly device: string; readonly id: number }
  | { readonly type: 'clip_dismissed' };

const baseName = (path: string): string => path.split(/[/\\]/).pop() ?? path;

/** `link` with the transfer `(device, id)` changed by `change` (a new one starts empty). */
function updateTransfer(
  link: LinkState,
  device: string,
  id: number,
  change: (t: Transfer) => Transfer,
): LinkState {
  const old = link.transfers.find((t) => t.device === device && t.id === id);
  const next = change(
    old ?? { device, id, file_name: '', done: 0, total: 0, state: { kind: 'active' } },
  );
  return {
    ...link,
    transfers: old ? link.transfers.map((t) => (t === old ? next : t)) : [...link.transfers, next],
  };
}

/** One linked-device event folded into `link`. */
export function applyLinkEvent(link: LinkState, event: EngineEvent): LinkState {
  switch (event.event) {
    case 'link_up':
    case 'link_down':
    case 'policy_changed':
      return { ...link, changes: link.changes + 1 };
    case 'ask_received':
      return {
        ...link,
        asks: [...link.asks, { id: event.ask, device: event.device, kind: event.kind }],
      };
    case 'ask_expired':
      return { ...link, asks: link.asks.filter((a) => a.id !== event.ask) };
    case 'clip_received':
      return { ...link, clips: link.clips + 1 };
    case 'clip_skipped':
      return { ...link, clip: { reason: event.reason, n: (link.clip?.n ?? 0) + 1 } };
    case 'file_progress':
      return updateTransfer(link, event.device, event.id, (t) => ({
        ...t,
        done: event.done,
        total: event.total,
      }));
    case 'file_saved':
      return updateTransfer(link, event.device, event.id, (t) => ({
        ...t,
        file_name: baseName(event.path),
        done: t.total,
        state: { kind: 'done' },
      }));
    case 'file_failed':
      return updateTransfer(link, event.device, event.id, (t) => ({
        ...t,
        state: { kind: 'failed', reason: event.reason },
      }));
    default:
      // Sent clips, Handoff and Ring need no note: the user just did them.
      return link;
  }
}

/** One user action on `link`. */
export function applyLinkAction(link: LinkState, action: LinkAction): LinkState {
  switch (action.type) {
    case 'ask_answered':
      return { ...link, asks: link.asks.filter((a) => a.id !== action.id) };
    case 'transfer_dismissed':
      return {
        ...link,
        transfers: link.transfers.filter((t) => t.device !== action.device || t.id !== action.id),
      };
    case 'clip_dismissed':
      return { ...link, clip: null };
  }
}
