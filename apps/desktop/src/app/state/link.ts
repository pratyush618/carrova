import type { SmsMessage } from '../../bindings/SmsMessage';
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
  /** The user closed its note; it stays (keeping its total) until it finishes. */
  readonly dismissed: boolean;
  /** This device sent it and the other side saved it. */
  readonly sent: boolean;
}

/** A clip note: this computer's clipboard kept here and why, or a device's that arrived. */
export type ClipNote =
  | { readonly kind: 'skipped'; readonly reason: ClipSkip }
  | { readonly kind: 'received'; readonly device: string };

/** A phone notification to pop up: its key only (the pop-up fetches it); `locked`: no content may show. */
export interface NoteAlert {
  readonly device: string;
  readonly key: string;
  readonly locked: boolean;
  /** Bumped per notification, so the same one twice pops up twice. */
  readonly n: number;
}

/** A text a phone received that it has not stored yet (`phone_sms` with id 0). */
export interface ProvisionalText {
  readonly device: string;
  readonly message: SmsMessage;
}

/** What linked devices did that the UI shows outside any page. */
export interface LinkState {
  readonly asks: readonly PendingAsk[];
  /** Oldest first; a finished one goes once dismissed. */
  readonly transfers: readonly Transfer[];
  /** The last clip note (arrivals only while the user asked for a note). */
  readonly clip: { readonly note: ClipNote; readonly n: number } | null;
  /** Bumped when a link or its features change: the device list is stale. */
  readonly changes: number;
  /** Bumped when a clip arrives: the history is stale. */
  readonly clips: number;
  /** The last phone notification to pop up. */
  readonly note: NoteAlert | null;
  /** Received texts the phones have not stored yet, for the conversation list. */
  readonly sms: readonly ProvisionalText[];
  /** Bumped when a phone's texts change: loaded conversations are stale. */
  readonly texts: number;
}

export const emptyLink: LinkState = {
  asks: [],
  transfers: [],
  clip: null,
  changes: 0,
  clips: 0,
  note: null,
  sms: [],
  texts: 0,
};

/** `link` without `device`'s provisional texts. */
const dropTexts = (link: LinkState, device: string): LinkState => ({
  ...link,
  sms: link.sms.filter((p) => p.device !== device),
  texts: link.texts + 1,
});

/** How far apart a provisional text and its stored copy may be dated (the engine's `STORED_SLACK_MS`). */
const STORED_SLACK_MS = 60_000;

/**
 * `link` once `device` stored `row`: without the provisional text it is the
 * copy of (same thread, body and direction; the nearest in time), as the
 * engine's mirror matches them. Other provisional texts stay.
 */
function storeText(link: LinkState, device: string, row: SmsMessage): LinkState {
  let at = -1;
  let best = STORED_SLACK_MS;
  link.sms.forEach(({ device: d, message: m }, i) => {
    const gap = Math.abs(m.when_ms - row.when_ms);
    const copy =
      d === device &&
      m.thread_id === row.thread_id &&
      m.text === row.text &&
      m.outgoing === row.outgoing;
    if (copy && gap <= best) [at, best] = [i, gap];
  });
  return { ...link, sms: link.sms.filter((_, i) => i !== at), texts: link.texts + 1 };
}

/** Things the user did to the link state (answered, dismissed). */
export type LinkAction =
  | { readonly type: 'ask_answered'; readonly id: AskId }
  | { readonly type: 'transfer_dismissed'; readonly device: string; readonly id: number }
  | { readonly type: 'clip_dismissed' };

const baseName = (path: string): string => path.split(/[/\\]/).pop() ?? path;

/** Finished and dismissed: nothing more will show it. */
const gone = (t: Transfer): boolean => t.dismissed && t.state.kind !== 'active';

/** `link` with the transfer `(device, id)` changed by `change` (a new one starts empty). */
function updateTransfer(
  link: LinkState,
  device: string,
  id: number,
  change: (t: Transfer) => Transfer,
): LinkState {
  const old = link.transfers.find((t) => t.device === device && t.id === id);
  const next = change(
    old ?? {
      device,
      id,
      file_name: '',
      done: 0,
      total: 0,
      state: { kind: 'active' },
      dismissed: false,
      sent: false,
    },
  );
  const transfers = old
    ? link.transfers.map((t) => (t === old ? next : t))
    : [...link.transfers, next];
  return { ...link, transfers: transfers.filter((t) => !gone(t)) };
}

/** `link` showing `note` (a new one each time, even if the same). */
const noteClip = (link: LinkState, note: ClipNote): LinkState => ({
  ...link,
  clip: { note, n: (link.clip?.n ?? 0) + 1 },
});

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
    case 'clip_received': {
      const noted = event.note ? noteClip(link, { kind: 'received', device: event.device }) : link;
      return { ...noted, clips: link.clips + 1 };
    }
    case 'clip_skipped':
      return noteClip(link, { kind: 'skipped', reason: event.reason });
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
    case 'file_sent':
      return updateTransfer(link, event.device, event.id, (t) => ({
        ...t,
        done: t.total,
        state: { kind: 'done' },
        sent: true,
      }));
    case 'file_failed':
      return updateTransfer(link, event.device, event.id, (t) => ({
        ...t,
        state: { kind: 'failed', reason: event.reason, retry: event.retry },
      }));
    case 'phone_note':
      return {
        ...link,
        note: {
          device: event.device,
          key: event.note.key,
          locked: event.note.hidden === 'locked',
          n: (link.note?.n ?? 0) + 1,
        },
      };
    case 'phone_sms':
      // A stored text replaces its provisional copy.
      return event.message.id === 0
        ? {
            ...link,
            sms: [...link.sms, { device: event.device, message: event.message }],
            texts: link.texts + 1,
          }
        : storeText(link, event.device, event.message);
    case 'phone_cleared':
      return event.feature === 'messages' ? dropTexts(link, event.device) : link;
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
    case 'transfer_dismissed': {
      // Older finished ones were behind it: they go with it.
      const at = link.transfers.findIndex((t) => t.device === action.device && t.id === action.id);
      const transfers = link.transfers.map((t, i) =>
        i === at || (i < at && t.state.kind !== 'active') ? { ...t, dismissed: true } : t,
      );
      return { ...link, transfers: transfers.filter((t) => !gone(t)) };
    }
    case 'clip_dismissed':
      return { ...link, clip: null };
  }
}
