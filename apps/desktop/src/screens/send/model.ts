import type { BridgeError } from '../../engine/types';
import type { OsKind } from '../../bindings/OsKind';
import type { Preset } from '../../bindings/Preset';
import type { RouteEnd } from '../../ui';
import type { AppRef, Bytes, ItemKey } from '../shared/model';
import type { TransferModel } from '../shared/transfer';

/** One app found on this computer (`carrova-view` selection row, app level). */
export interface FoundApp extends AppRef {
  /** What moves, from the recipe ("Name, email, aliases, global ignore"). */
  readonly what: string;
  readonly files: number;
  readonly bytes: Bytes;
  readonly included: boolean;
}

/** Apps of one recipe category (`dev-tools`), in the scan's order. */
export interface FoundGroup {
  readonly category: string;
  readonly apps: readonly FoundApp[];
}

/** A key or secret (`carrova-view` sensitive item): starts off, brought only after its own confirm. */
export interface SecretItem {
  readonly key: ItemKey;
  readonly app: AppRef;
  readonly label: string;
  /** The recipe's reason, literal; `null` uses the general wording for secrets. */
  readonly why: string | null;
  /** Its declared paths as the user knows them (the key files). */
  readonly paths: readonly string[];
  readonly bytes: Bytes;
  readonly brought: boolean;
}

/** The move so far, in the rail's words ("11 of 15 apps", "Keys left behind"). */
export interface MoveFacts {
  readonly apps: number;
  readonly totalApps: number;
  /** Everything ticked: apps, keys and raw folders. */
  readonly bytes: Bytes;
  /** Secrets being brought. */
  readonly keys: number;
  readonly caches: boolean;
}

/** The presets the selection screen offers, in order ("Suggested · Everything · Nothing"). */
export const FOUND_PRESETS = [
  'suggested',
  'everything',
  'none',
] as const satisfies readonly Preset[];
export type FoundPreset = (typeof FOUND_PRESETS)[number];

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
      /** Groups and secrets matching `filter` (all of them when it's empty). */
      readonly groups: readonly FoundGroup[];
      readonly secrets: readonly SecretItem[];
      /** The view filter's text: front-end only, the selection ignores it. */
      readonly filter: string;
      /** The preset the selection still matches; `null` once edited. */
      readonly preset: FoundPreset | null;
      /** Folders no recipe covers (any size): they can come too, as plain files. */
      readonly unrecognised: number;
      readonly facts: MoveFacts;
      /** Why the selection can't be used yet (engine wording); Continue waits for none. */
      readonly issues: readonly string[];
    };

/** Whether the selection leads to pairing (send) or a file (save). */
export type FoundPurpose = 'send' | 'save';

export interface FoundIntents {
  readonly onToggleApp: (id: string, included: boolean) => void;
  /** Asks to bring a secret: opens its confirm. */
  readonly onConfirmKey: (key: ItemKey) => void;
  readonly onLeaveKey: (key: ItemKey) => void;
  readonly onFilter: (text: string) => void;
  /** Start over from a preset (keys keep their own answers). */
  readonly onPreset: (preset: FoundPreset) => void;
  readonly onCustomize: () => void;
  readonly onPickFolders: () => void;
  readonly onSeeKnownApps: () => void;
  /** Sending only: keep the selection and save it to a file instead. */
  readonly onSaveInstead?: () => void;
  readonly onContinue: () => void;
  readonly onBack: () => void;
}

/** The secret's confirm: bring it, or leave it (the safe choice). */
export interface KeyConfirmIntents {
  readonly onBring: () => void;
  readonly onLeave: () => void;
}

/** This computer and the other one, plus what moves: the rail and the route chip. */
export interface SendMove {
  /** `null` until the engine has said which computer this is. */
  readonly here: RouteEnd | null;
  /** `null` until the scan is known (a resumed send has none). */
  readonly facts: MoveFacts | null;
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

/** Paired: the code was proven on both sides; the receiver now reviews the plan. */
export interface LinkedModel extends SendMove {
  /** The receiver: its OS once its inventory arrives. */
  readonly peer: { readonly name: string; readonly os: OsKind | null };
  /** How many of the receiver's files the plan replaces (0 until they share it). */
  readonly replaced: number;
}

export interface LinkedIntents {
  readonly onCancel: () => void;
}

export interface DeliveredModel {
  readonly peer: string;
  readonly apps: number;
  readonly bytes: Bytes;
}

export interface DeliveredIntents {
  readonly onDetails: () => void;
  readonly onDone: () => void;
}
