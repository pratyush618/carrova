import type { BridgeError, Retention } from '../../engine/types';
import type { AppRef, AskAnswer, Bytes, ConflictRule, Failure, ItemKey } from '../shared/model';
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
  | { readonly status: 'connecting'; readonly here: string; readonly peer: string }
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
    };

export type DecisionAnswer = NonNullable<DecisionModel['answer']>;

/** "Here's what will change": the approval gate (`carrova-view` plan + space views). */
export interface ReviewModel {
  readonly counts: {
    readonly added: number;
    readonly replaced: number;
    readonly identical: number;
  };
  /** Undo window in days; `null` keeps the undo point until the user removes it. */
  readonly undoDays: number | null;
  readonly decisions: readonly DecisionModel[];
  readonly space: {
    readonly needed: Bytes;
    /** `null` when the free space couldn't be measured. */
    readonly free: Bytes | null;
    readonly short: boolean;
    /** The user chose to ignore the space check. */
    readonly ignored: boolean;
  };
  /** What still stops the plan; Approve waits for none. */
  readonly blockers: readonly BlockerModel[];
  /** A change is being re-planned; Approve waits. */
  readonly busy: boolean;
}

/** One engine blocker (`Blocker`), with names and sizes ready to show. */
export type BlockerModel =
  | { readonly kind: 'unconfirmed_sensitive'; readonly what: string; readonly why: string }
  | { readonly kind: 'running_app'; readonly app: string }
  | { readonly kind: 'free_space'; readonly needed: Bytes; readonly free: Bytes };

export interface ReviewIntents {
  readonly onAnswer: (key: ItemKey, answer: DecisionAnswer) => void;
  readonly onApplyWhileOpen: (key: ItemKey) => void;
  readonly onIgnoreSpace: () => void;
  readonly onSeeEvery: () => void;
  readonly onCancel: () => void;
  readonly onApprove: () => void;
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

/** Something left for the user after a move (`Checklist`). */
export type Task =
  | {
      readonly kind: 'reinstall';
      readonly app: string;
      readonly count: number;
      readonly command: string;
    }
  /** An app whose data arrived but which isn't installed here. */
  | { readonly kind: 'install'; readonly app: string }
  | { readonly kind: 'new-terminal' }
  | { readonly kind: 'sign-in'; readonly app: string };

export interface ArrivedModel {
  readonly peer: string;
  readonly apps: number;
  readonly appsTotal: number;
  readonly bytes: Bytes;
  readonly replaced: number;
  readonly failures: readonly Failure[];
  readonly tasks: readonly Task[];
}

export interface ArrivedIntents {
  readonly onCopyCommand: (command: string) => void;
  /** Offered when failed items can be tried again. */
  readonly onRetry?: () => void;
  /** Offered while a rollback point exists. */
  readonly onUndo?: () => void;
  readonly onDone: () => void;
}
