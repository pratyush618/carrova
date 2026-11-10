import type { BridgeError, KdfStrength } from '../../engine/types';
import type { Strength } from '../../ui';
import type { Bytes } from '../shared/model';

/** Where the file goes and what locks it. */
export interface SaveLockModel {
  readonly passphrase: string;
  readonly confirm: string;
  readonly strength: Strength;
  /** Destination path, typed (or picked with the native dialog when offered). */
  readonly path: string;
  /** Passphrase protection level (the engine's; never below its default). */
  readonly kdf: KdfStrength;
  /** A native "save as" dialog exists. */
  readonly canChoose: boolean;
  readonly saving: boolean;
  /** Strong enough (the engine's policy), confirmed, with a destination, not saving yet. */
  readonly canSave: boolean;
  readonly error: BridgeError | null;
}

export interface SaveLockIntents {
  readonly onPassphrase: (value: string) => void;
  readonly onConfirm: (value: string) => void;
  readonly onPath: (value: string) => void;
  readonly onKdf: (value: KdfStrength) => void;
  readonly onChoosePath: () => void;
  readonly onBack: () => void;
  readonly onSave: () => void;
}

export interface SavedModel {
  readonly path: string;
  readonly bytes: Bytes;
}

export interface SavedIntents {
  /** Offered only where the system file manager can be opened. */
  readonly onShowInFolder?: () => void;
  readonly onDone: () => void;
}

export interface OpenFileModel {
  /** The file to open, typed (or picked with the native dialog when offered). */
  readonly path: string;
  /** A native "open" dialog exists. */
  readonly canChoose: boolean;
  readonly passphrase: string;
  /** The last attempt didn't decrypt the file. */
  readonly wrong: boolean;
  readonly opening: boolean;
  /** Any other failure (missing file, damaged archive). */
  readonly error: BridgeError | null;
}

export interface OpenFileIntents {
  readonly onPath: (value: string) => void;
  readonly onChooseAnother: () => void;
  readonly onPassphrase: (value: string) => void;
  readonly onBack: () => void;
  readonly onOpen: () => void;
}
