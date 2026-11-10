/**
 * View-model building blocks shared by every flow. Shapes follow the engine
 * (`carrova-view`, `carrova-core`); screens get them already resolved and only
 * format and render.
 */

/** A size in bytes. Formatted with `useFormat().bytes`. */
export type Bytes = number;

import type { OsKind } from '../../engine/types';

export type { ConflictRule, ItemKey } from '../../engine/types';

/** An app as every screen shows it: monogram and name (from its recipe). */
export interface AppRef {
  readonly id: string;
  readonly glyph: string;
  readonly name: string;
}

/** A sensitive item's answer. Sensitive items always start on `leave`. */
export type AskAnswer = 'leave' | 'bring';

/** Something that did not happen, and the engine's reason. */
export interface Failure {
  readonly what: string;
  readonly reason: string;
}

/** One computer of a move, as far as it is known (an archive may not name its source). */
export interface Computer {
  /** Its device name; `null` when it never said. */
  readonly name: string | null;
  /** Its OS family; `null` until known. */
  readonly os: OsKind | null;
}

/** Where a move goes: the rail's route and the live card's two ends. */
export interface MoveRoute {
  readonly from: Computer;
  readonly to: Computer;
}
