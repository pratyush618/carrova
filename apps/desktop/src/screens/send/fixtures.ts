import { APPS, GB, KB, MB, RECEIVER, SENDER } from '../shared/fixtures';
import type { RouteEnd } from '../../ui';
import type { AppRef } from '../shared/model';
import type {
  CustomizeModel,
  DeliveredModel,
  FoundApp,
  FoundGroup,
  FoundModel,
  LinkedModel,
  LiveModel,
  MoveFacts,
  PairingModel,
  SecretItem,
  SendMove,
  UnrecognisedModel,
} from './model';
import { transferAt } from '../shared/transferFixtures';

const app = (ref: AppRef, what: string, files: number, bytes = 0): FoundApp => ({
  ...ref,
  what,
  files,
  bytes,
  included: true,
});

const GROUPS: readonly FoundGroup[] = [
  {
    category: 'dev-tools',
    apps: [app(APPS.git, 'Name, email, aliases, global ignore', 2, 4 * KB)],
  },
  { category: 'editors', apps: [app(APPS.vscode, 'Settings, keybindings, snippets', 388, GB)] },
  {
    category: 'security',
    apps: [
      app(APPS.ssh, 'Server list and shortcuts', 3, 14 * KB),
      { ...app(APPS.gpg, 'Public keys and trust settings', 4, 96 * KB), included: false },
    ],
  },
  {
    category: 'shells',
    apps: [app(APPS.shell, 'History, aliases and prompt theme', 15, 2.3 * MB)],
  },
];

export const SECRETS: readonly SecretItem[] = [
  {
    key: 'ssh/keys',
    app: APPS.ssh,
    label: 'SSH key pairs (private keys)',
    why: 'They log you in to servers and GitHub. Anyone with a copy can do the same, as you.',
    paths: ['~/.ssh/id_ed25519', '~/.ssh/id_ed25519_work'],
    bytes: 6 * KB,
    brought: false,
  },
  {
    key: 'gpg/secret-keys',
    app: APPS.gpg,
    label: 'GnuPG secret keys',
    why: 'They sign your commits and open mail sent to you. Anyone with a copy can do both.',
    paths: ['~/.gnupg/private-keys-v1.d'],
    bytes: 12 * KB,
    brought: false,
  },
  {
    key: 'git/credentials',
    app: APPS.git,
    label: 'Stored Git credentials',
    why: null,
    paths: ['~/.git-credentials'],
    bytes: 1 * KB,
    brought: false,
  },
];

export const HERE: RouteEnd = { os: 'linux', name: SENDER };

const FACTS: MoveFacts = { apps: 4, totalApps: 5, bytes: 1.2 * GB, keys: 0, caches: false };

export const move: SendMove = { here: HERE, facts: FACTS };

export const foundScanning: FoundModel = {
  status: 'scanning',
  foundSoFar: ['Git', 'SSH', 'GnuPG'],
  progress: 0.45,
};
export const foundReady = {
  status: 'found',
  groups: GROUPS,
  secrets: SECRETS,
  filter: '',
  preset: 'suggested',
  unrecognised: 3,
  facts: FACTS,
  issues: [],
} satisfies FoundModel;
export const foundNothing: FoundModel = { status: 'nothing', knownApps: 15 };

export const customize: CustomizeModel = {
  filter: '',
  apps: [
    {
      ...APPS.ssh,
      bytes: 14 * KB,
      expanded: true,
      items: [
        {
          key: 'ssh/config',
          label: 'Server list and shortcuts',
          detail: { kind: 'path', path: '~/.ssh/config' },
          bytes: 6 * KB,
          picked: true,
          tag: { kind: 'suggested' },
          paths: [],
        },
        {
          key: 'ssh/known-hosts',
          label: 'Known servers',
          detail: { kind: 'path', path: '~/.ssh/known_hosts' },
          bytes: 8 * KB,
          picked: true,
          tag: { kind: 'suggested' },
          paths: [],
        },
        {
          key: 'ssh/keys',
          label: 'SSH keys',
          detail: { kind: 'files', count: 4 },
          bytes: 9 * KB,
          picked: false,
          tag: { kind: 'sensitive', confirmed: false },
          paths: [],
        },
        {
          key: 'ssh/authorized-keys',
          label: 'Keys allowed to log in here',
          detail: { kind: 'path', path: '~/.ssh/authorized_keys' },
          bytes: 1 * KB,
          picked: false,
          tag: { kind: 'sensitive', confirmed: false },
          paths: [],
        },
      ],
    },
    {
      ...APPS.git,
      bytes: 4 * KB,
      expanded: true,
      items: [
        {
          key: 'git/config',
          label: 'Name, email and aliases',
          detail: { kind: 'path', path: '~/.gitconfig' },
          bytes: 3 * KB,
          picked: true,
          tag: { kind: 'suggested' },
          paths: [],
        },
        {
          key: 'git/credentials',
          label: 'Saved passwords and tokens',
          detail: { kind: 'path', path: '~/.git-credentials' },
          bytes: 1 * KB,
          picked: false,
          tag: { kind: 'sensitive', confirmed: false },
          paths: [],
        },
      ],
    },
    {
      ...APPS.vscode,
      bytes: 1.2 * GB,
      expanded: true,
      items: [
        {
          key: 'vscode/settings',
          label: 'Settings and keybindings',
          detail: { kind: 'path', path: 'user_config/Code/User' },
          bytes: 212 * KB,
          picked: true,
          tag: { kind: 'suggested' },
          paths: [],
        },
        {
          key: 'vscode/extensions',
          label: 'Extensions',
          detail: { kind: 'extensions', count: 12 },
          bytes: 640 * MB,
          picked: false,
          tag: {
            kind: 'unmovable',
            reason: 'Built for this system. Reinstall instead.',
            forced: false,
          },
          paths: [],
        },
        {
          key: 'vscode/cache',
          label: 'Cache',
          detail: { kind: 'path', path: 'cache/Code' },
          bytes: 380 * MB,
          picked: false,
          tag: null,
          paths: [],
        },
      ],
    },
  ],
  unrecognised: { count: 3, minBytes: 10 * MB },
  includeCaches: false,
  cacheBytes: 412 * MB,
  picked: { files: 398, totalFiles: 418, bytes: 1.2 * GB },
};

export const unrecognised: UnrecognisedModel = {
  minSize: '10mb',
  folders: [
    {
      key: 'raw:home/projects/notes',
      path: '~/projects/notes',
      hint: 'Markdown notes',
      bytes: 84 * MB,
      picked: false,
    },
    {
      key: 'raw:user_data/fonts',
      path: '~/.local/share/fonts',
      hint: 'Fonts you installed',
      bytes: 31 * MB,
      picked: false,
    },
    {
      key: 'raw:user_config/obsidian',
      path: '~/.config/obsidian',
      hint: "An app Carrova doesn't know yet",
      bytes: 12 * MB,
      picked: false,
    },
  ],
};

const CODE = ['7', 'zebra', 'lemon'];
const ADDRESS = '7-zebra-lemon@192.168.1.20:47822';

export const pairingWaiting: PairingModel = { status: 'waiting', code: CODE, address: ADDRESS };
export const pairingCantConnect: PairingModel = {
  status: 'cant-connect',
  code: CODE,
  address: ADDRESS,
};
export const pairingRenewed: PairingModel = {
  status: 'renewed',
  code: ['7', 'acorn', 'maple'],
  address: '7-acorn-maple@192.168.1.20:47822',
};
export const pairingLocked: PairingModel = { status: 'locked' };

/** Paired with a Mac: the cross-OS banner shows. */
export const linkedCrossOs: LinkedModel = {
  ...move,
  peer: { name: RECEIVER, os: 'mac_os' },
  replaced: 3,
};
export const linkedSameOs: LinkedModel = {
  ...linkedCrossOs,
  peer: { name: RECEIVER, os: 'linux' },
};
export const liveMoving = {
  status: 'transfer',
  transfer: transferAt('moving', 'send'),
} satisfies LiveModel;
export const liveReconnecting = {
  status: 'transfer',
  transfer: transferAt('reconnecting', 'send'),
} satisfies LiveModel;

export const delivered: DeliveredModel = { peer: RECEIVER, apps: 5, bytes: 1.2 * GB };
