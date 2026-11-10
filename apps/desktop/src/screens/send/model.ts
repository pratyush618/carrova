import type { BridgeError } from '../../engine/types';
import type { AppRef, AskAnswer, Bytes, ItemKey } from '../shared/model';
import type { TransferModel } from '../shared/transfer';

/** One app found on this computer (`carrova-view` selection row, app level). */
export interface FoundApp extends AppRef {
  /** What moves, from the recipe ("Name, email, aliases, global ignore"). */
  readonly what: string;
  readonly files: number;
  readonly bytes: Bytes;
  readonly included: boolean;
}

/** Why an item needs an explicit OK. */
export type AskKind = 'secret' | 'runs-code';

/** A sensitive item, asked in plain words (`carrova-view` `Sensitivity`). */
export interface SensitiveAsk {
  readonly key: ItemKey;
  readonly title: string;
  /** The recipe's reason, literal; `null` uses the general wording for secrets. */
  readonly why: string | null;
  readonly kind: AskKind;
  readonly answer: AskAnswer;
}

/** "Here's what we found": scanning, nothing, or the suggested selection. */
export type FoundModel =
  | {
      readonly status: 'scanning';
      readonly foundSoFar: readonly string[];
      /** 0–1 when the engine reports it; `null` shows no bar (never a fake one). */
      readonly progress: number | null;
    }
  | { readonly status: 'nothing'; readonly knownApps: number }
  | {
      readonly status: 'found';
      readonly apps: readonly FoundApp[];
      readonly asks: readonly SensitiveAsk[];
      /** Why the selection can't be used yet (engine wording); Continue waits for none. */
      readonly issues: readonly string[];
    };

/** Whether the selection leads to pairing (send) or a file (save). */
export type FoundPurpose = 'send' | 'save';

export interface FoundIntents {
  readonly onToggleApp: (id: string, included: boolean) => void;
  readonly onAnswer: (key: ItemKey, answer: AskAnswer) => void;
  readonly onCustomize: () => void;
  readonly onPickFolders: () => void;
  readonly onSeeKnownApps: () => void;
  readonly onContinue: () => void;
  readonly onBack: () => void;
}

/** A tag on an item (`carrova-view` `SelectionTag`). */
export type ItemTag =
  | { readonly kind: 'suggested' }
  | { readonly kind: 'sensitive'; readonly confirmed: boolean }
  | { readonly kind: 'unmovable'; readonly reason: string; readonly forced: boolean };

/** The line under an item's name. */
export type ItemDetail =
  | { readonly kind: 'path'; readonly path: string }
  | { readonly kind: 'files'; readonly count: number }
  | { readonly kind: 'extensions'; readonly count: number };

/** One selectable item of an app (Customize tree, level 2). */
export interface TreeItem {
  readonly key: ItemKey;
  readonly label: string;
  readonly detail: ItemDetail;
  readonly bytes: Bytes;
  readonly picked: boolean;
  readonly tag: ItemTag | null;
  /** The item's declared paths when it has several (level 3); empty otherwise. */
  readonly paths: readonly TreePath[];
}

/** One declared path under an item (`path` is the engine key `root:rel`). */
export interface TreePath {
  readonly path: string;
  readonly display: string;
  readonly picked: boolean;
}

/** One app of the Customize tree (level 1). */
export interface TreeApp extends AppRef {
  readonly bytes: Bytes;
  readonly expanded: boolean;
  readonly items: readonly TreeItem[];
}

/** The full selection, one level down from the summary. */
export interface CustomizeModel {
  readonly filter: string;
  readonly apps: readonly TreeApp[];
  readonly unrecognised: { readonly count: number; readonly minBytes: Bytes };
  readonly includeCaches: boolean;
  readonly cacheBytes: Bytes;
  readonly picked: { readonly files: number; readonly totalFiles: number; readonly bytes: Bytes };
}

export interface CustomizeIntents {
  readonly onFilter: (text: string) => void;
  readonly onToggleApp: (id: string, picked: boolean) => void;
  readonly onToggleItem: (key: ItemKey, picked: boolean) => void;
  readonly onTogglePath: (key: ItemKey, path: string, picked: boolean) => void;
  readonly onExpand: (id: string) => void;
  readonly onIncludeAnyway: (key: ItemKey) => void;
  readonly onUnrecognised: () => void;
  readonly onIncludeCaches: (on: boolean) => void;
  readonly onReset: () => void;
  readonly onDone: () => void;
}

/** Minimum-size filter for unrecognised folders. */
export type MinSize = '10mb' | '100mb' | 'any';

/** A home folder no recipe covers. */
export interface RawFolder {
  readonly key: ItemKey;
  readonly path: string;
  readonly hint: string;
  readonly bytes: Bytes;
  readonly picked: boolean;
}

export interface UnrecognisedModel {
  readonly minSize: MinSize;
  readonly folders: readonly RawFolder[];
}

export interface UnrecognisedIntents {
  readonly onMinSize: (size: MinSize) => void;
  readonly onToggle: (key: ItemKey, picked: boolean) => void;
  readonly onDone: () => void;
}

/** Pairing (`carrova-view` `CodeView`): a live code, or stopped after repeated wrong codes. */
export type PairingModel =
  /** Capturing and starting to listen; the code follows in a moment. */
  | { readonly status: 'preparing' }
  | {
      readonly status: 'waiting' | 'cant-connect' | 'renewed';
      readonly code: readonly string[];
      /** Fallback address, typed on the other computer when discovery is blocked; `null` if none. */
      readonly address: string | null;
    }
  | { readonly status: 'locked' }
  /** Pairing or the send ended with an error other than wrong codes. */
  | { readonly status: 'failed'; readonly error: BridgeError };

export interface PairingIntents {
  readonly onBack: () => void;
  readonly onNewCode: () => void;
  readonly onHome: () => void;
}

/** "A wrong code was tried": the sender decides whether to show a new one. */
export interface NewCodeModel {
  readonly failures: number;
  readonly remaining: number;
}

export interface NewCodeIntents {
  readonly onStop: () => void;
  readonly onNewCode: () => void;
}

/** Connected: the receiver is reviewing, then the transfer runs. */
export type LiveModel =
  | {
      readonly status: 'reviewing';
      readonly here: string;
      readonly peer: string;
      readonly replaced: number;
    }
  | { readonly status: 'transfer'; readonly transfer: TransferModel };

export interface DeliveredModel {
  readonly peer: string;
  readonly apps: number;
  readonly bytes: Bytes;
}

export interface DeliveredIntents {
  readonly onDetails: () => void;
  readonly onDone: () => void;
}
