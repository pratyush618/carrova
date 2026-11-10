/** One rule the user may bend, behind a warning that states the reason. */
export type OverrideModel =
  | {
      readonly kind: 'unmovable';
      /** "VS Code extensions". */
      readonly item: string;
      /** The recipe's reason it doesn't move. */
      readonly reason: string;
      readonly count: number;
    }
  | { readonly kind: 'running'; readonly app: string }
  | { readonly kind: 'space' }
  | { readonly kind: 'elsewhere' };

export interface OverrideIntents {
  /** The safe choice: closes without changing anything. Focused first. */
  readonly onKeepSafe: () => void;
  readonly onOverride: () => void;
}
