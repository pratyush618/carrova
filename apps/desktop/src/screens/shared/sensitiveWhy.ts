import type { ItemKey } from '../../engine/types';

/**
 * Why a sensitive item is asked about, as a key into the `sensitive.*`
 * copy. The engine's own reason ("marked sensitive by recipe …") is never
 * shown; `secret` is the general wording for items not listed here.
 */
export type SensitiveWhy =
  'login' | 'authorized' | 'signing' | 'token' | 'remote' | 'variables' | 'history' | 'secret';

/** Every recipe item flagged `sensitive` (recipes/**), by what a copy lets someone do. */
const WHY: Readonly<Record<ItemKey, SensitiveWhy>> = {
  'ssh/keys': 'login',
  'ssh/authorized-keys': 'authorized',
  'gnupg/secret-keys': 'signing',
  'git/credentials': 'token',
  'gh/hosts': 'token',
  'cargo/credentials': 'token',
  'npm/npmrc': 'token',
  'docker/config': 'token',
  'docker/context-tls': 'remote',
  'fish/universal-variables': 'variables',
  'bash/history': 'history',
  'fish/history': 'history',
  'powershell/history': 'history',
};

/** The human reason for a sensitive item (`ssh/keys` → `login`), else the general one. */
export const sensitiveWhy = (key: ItemKey): SensitiveWhy => WHY[key] ?? 'secret';
