import { GB, SENDER } from '../shared/fixtures';
import type { OpenFileModel, SavedModel, SaveLockModel } from './model';

const FILE_NAME = `carrova-${SENDER}-2026-11-08.carrova`;

const lock = (passphrase: string, strength: SaveLockModel['strength']): SaveLockModel => ({
  passphrase,
  confirm: '',
  strength,
  path: `~/${FILE_NAME}`,
  kdf: 'standard',
  canChoose: true,
  saving: false,
  canSave: false,
  error: null,
});

export const lockStrong = lock('correct horse battery staple', 'strong');
export const lockWeak = lock('summer', 'weak');

export const saved: SavedModel = { path: `~/${FILE_NAME}`, bytes: 1.2 * GB };

const open = (wrong: boolean): OpenFileModel => ({
  path: `~/${FILE_NAME}`,
  canChoose: true,
  passphrase: 'hunter2hunter2',
  wrong,
  opening: false,
  error: null,
});

export const openReady = open(false);
export const openWrong = open(true);
