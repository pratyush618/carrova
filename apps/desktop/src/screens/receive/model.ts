import type { BridgeError, OsKind, Retention, SemanticRoot } from '../../engine/types';
import type {
  AppRef,
  AskAnswer,
  Bytes,
  ConflictRule,
  Failure,
  ItemKey,
  MoveRoute,
} from '../shared/model';
import type { TransferModel } from '../shared/transfer';

/** Typing the code, the handshake, or a recoverable failure. */
export type CodeEntryModel =
  | {
      readonly status: 'typing' | 'not-found' | 'mismatch';
      readonly value: string;
      /** The sender's name, known after a failed attempt. */
      readonly peer: string | null;
      /** Pairing words to autocomplete from. */
      readonly words: readonly string[];
      /** Not found: the address fallback field. */
      readonly address: string;
    }
  | {
      readonly status: 'connecting';
      readonly here: string;
      readonly peer: string;
      /** The code as typed, in words (`['7', 'zebra', 'lemon']`). */
      readonly code: readonly string[];
    }
  /** Pairing failed for another reason (engine wording). */
  | {
      readonly status: 'failed';
      readonly value: string;
      readonly words: readonly string[];
      readonly error: BridgeError;
    };

export interface CodeEntryIntents {
  readonly onChange: (value: string) => void;
  readonly onAddress: (value: string) => void;
  readonly onConnect: () => void;
  readonly onCancel: () => void;
  readonly onBack: () => void;
}

/** A decision the receiver must answer before approving. */
export type DecisionModel =
  | {
      readonly kind: 'secret';
      readonly key: ItemKey;
      readonly title: string;
      readonly why: string;
      readonly answer: AskAnswer | null;
    }
  | {
      readonly kind: 'app-running';
      readonly key: ItemKey;
      readonly app: string;
      readonly answer: 'skip' | 'closed' | null;
    }
  /** A same-system item from another system: leave it, or bring it anyway (override). */
  | {
      readonly kind: 'cross-os';
      readonly key: ItemKey;
      readonly title: string;
      readonly from: OsKind;
      readonly to: OsKind;
      readonly answer: AskAnswer | null;
    }
  /** A plugin would change some items' files: allow it (consent dialog) or leave them. */
  | {
      readonly kind: 'plugin';
      /** The plugin id. */
      readonly key: string;
      readonly description: string;
      /** The items it changes, by name. */
      readonly items: readonly string[];
      readonly answer: 'allow' | 'leave' | null;
    };

/** Answers given in place on the review screen (the rest go through a dialog). */
export type DecisionAnswer = AskAnswer | 'skip' | 'closed';

/** "Here's what will change": the approval gate (`carrova-view` plan + space views). */
export interface ReviewModel {
  readonly counts: {
    readonly added: number;
    readonly replaced: number;
    readonly identical: number;
    /** Offered items that stay behind. */
    readonly left: number;
  };
  readonly decisions: readonly DecisionModel[];
  readonly space: {
    readonly needed: Bytes;
    /** `null` when the free space couldn't be measured. */
    readonly free: Bytes | null;
    readonly short: boolean;
    /** The user chose to ignore the space check. */
    readonly ignored: boolean;
  };
  /** Cross-system items land at this computer's paths; `null` when the source runs this OS. */
  readonly mapped: { readonly from: OsKind; readonly to: OsKind; readonly apps: number } | null;
  /** The suggestion for clashes (Every change can change it per app or file). */
  readonly rule: ConflictRule;
  readonly retention: Retention;
  /** The engine still blocks the plan; Approve waits. */
  readonly blocked: boolean;
  readonly rail: ReviewRail;
  /** A change is being re-planned; Approve waits. */
  readonly busy: boolean;
}

/** The review's "This move" rail. */
export interface ReviewRail {
  readonly route: MoveRoute;
  /** Apps with something moving, of the apps offered. */
  readonly apps: { readonly moving: number; readonly offered: number };
  readonly bytes: Bytes;
  /** Sensitive items brought, by name; `null` while one is unanswered. */
  readonly keys: readonly string[] | null;
}

export interface ReviewIntents {
  readonly onAnswer: (key: ItemKey, answer: DecisionAnswer) => void;
  readonly onApplyWhileOpen: (key: ItemKey) => void;
  readonly onIgnoreSpace: () => void;
  /** Bring a same-system item from another system anyway (opens its warning). */
  readonly onBringAnyway: (key: ItemKey) => void;
  /** Show what a plugin may do and ask to allow it. */
  readonly onReviewPlugin: (plugin: string) => void;
  /** Where cross-system items land (the mapping sheet). */
  readonly onSeeWhere: () => void;
  readonly onSeeEvery: () => void;
  readonly onCancel: () => void;
  readonly onApprove: () => void;
}

/** One cross-system item slot: where it was there and where it lands here. */
export interface MappingRow {
  readonly key: string;
  readonly glyph: string;
  readonly title: string;
  readonly from: MappedPath;
  readonly to: MappedPath;
}

/** A path as a semantic root + relative path, and the real path when known here. */
export interface MappedPath {
  readonly root: SemanticRoot;
  readonly rel: string;
  /** The absolute path on this computer (only the target side can know it). */
  readonly abs: string | null;
}

/** "Where things land": rows that change place first, same-place rows folded. */
export interface MappingModel {
  readonly from: OsKind;
  readonly to: OsKind;
  readonly moved: readonly MappingRow[];
  readonly same: readonly MappingRow[];
}

/** What a plugin asks to do, from its manifest (`PlannedPlugin`). */
export interface PluginConsentModel {
  readonly plugin: string;
  readonly version: string;
  readonly description: string;
  /** One plain sentence per permission. */
  readonly permissions: readonly string[];
  /** The items it changes, by name. */
  readonly items: readonly string[];
}

export interface PluginConsentIntents {
  /** Allow it for this move only. */
  readonly onAllow: () => void;
  /** Don't allow: its items are left behind. */
  readonly onDecline: () => void;
  /** Close without answering. */
  readonly onClose: () => void;
}

/** A clashing file and the rule for it; `null` follows the item's rule. */
export interface Clash {
  /** Engine key of the target (`root:rel`), as per-file rules name it. */
  readonly path: string;
  /** Where it is on this computer. */
  readonly display: string;
  readonly rule: ConflictRule | null;
}

/** Every change for one item (`carrova-view` `ItemView`); `id` is the item key. */
export interface ChangeGroup extends AppRef {
  readonly rule: ConflictRule;
  readonly clashes: readonly Clash[];
  readonly added: number;
  /** Whether "Put elsewhere…" is offered (remap, re-confined on this computer). */
  readonly canRemap: boolean;
  /** Restored somewhere else (remapped), shown so it's never a surprise. */
  readonly remapped: boolean;
}

/** Import-wide choices; each starts on the engine's suggestion. */
export interface ImportChoices {
  /** Name added to the copy when both are kept (`{date}` becomes the day). */
  readonly keepBothSuffix: string;
  readonly retention: Retention;
  /** Also import trust settings (GnuPG owner trust): off, since it can trust a stranger's key. */
  readonly importTrust: boolean;
  /** Keep group-write permission bits from the other computer. */
  readonly keepGroupWrite: boolean;
}

export interface ReviewDetailsModel {
  readonly groups: readonly ChangeGroup[];
  readonly choices: ImportChoices;
}

export interface ReviewDetailsIntents {
  readonly onRule: (item: ItemKey, rule: ConflictRule) => void;
  readonly onFileRule: (item: ItemKey, path: string, rule: ConflictRule | null) => void;
  readonly onRemap: (item: ItemKey) => void;
  readonly onChoices: (patch: Partial<ImportChoices>) => void;
  readonly onDone: () => void;
}

/** Transfer + apply, or a failed apply that was rolled back. */
export type ReceiveProgressModel =
  | { readonly status: 'transfer'; readonly transfer: TransferModel }
  /** Apply failed and was undone automatically; the engine says why. */
  | { readonly status: 'failed'; readonly error: BridgeError };

export interface ReceiveProgressIntents {
  /** Offered when the session can be picked up again. */
  readonly onRetry?: () => void;
  readonly onSeeWhat: () => void;
  readonly onHome: () => void;
}

/** After the move: what arrived, what didn't, and what is left to install. */
export interface ArrivedModel {
  readonly route: MoveRoute;
  /** Apps that arrived, by name. */
  readonly moved: readonly string[];
  readonly appsTotal: number;
  readonly files: number;
  readonly bytes: Bytes;
  readonly replaced: number;
  readonly failures: readonly Failure[];
  /** Apps whose settings arrived but which aren't installed here. */
  readonly toInstall: readonly string[];
}

export interface ArrivedIntents {
  /** Offered when failed items can be tried again. */
  readonly onRetry?: () => void;
  /** Offered while a rollback point exists. */
  readonly onUndo?: () => void;
  readonly onDone: () => void;
}
