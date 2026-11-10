/** First run. */
export interface WelcomeModel {
  /** Crash-report consent: off until the user turns it on. */
  readonly errorReports: boolean;
}

export interface WelcomeIntents {
  readonly onErrorReports: (on: boolean) => void;
  readonly onStart: () => void;
}

/** The most recent move, shown on Home with its Undo. */
export interface LastMove {
  readonly direction: 'received' | 'sent';
  readonly peer: string;
  readonly when: Date;
  readonly apps: number;
  readonly undoable: boolean;
}

export interface HomeModel {
  readonly now: Date;
  readonly last: LastMove | null;
}

export interface HomeIntents {
  readonly onSend: () => void;
  readonly onReceive: () => void;
  readonly onUseFile: () => void;
  readonly onUndoLast: () => void;
}

export interface FileChooseIntents {
  readonly onSave: () => void;
  readonly onOpen: () => void;
  readonly onCancel: () => void;
}
