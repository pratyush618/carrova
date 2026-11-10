/**
 * View-model building blocks shared by every flow. Shapes follow the engine
 * (`carrova-view`, `carrova-core`); screens get them already resolved and only
 * format and render.
 */

/** A size in bytes. Formatted with `useFormat().bytes`. */
export type Bytes = number;

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
