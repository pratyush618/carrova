import { APPS, GB, KB, MB, RECEIVER, SENDER } from '../shared/fixtures';
import type {
  CustomizeModel,
  DeliveredModel,
  FoundApp,
  FoundModel,
  LiveModel,
  PairingModel,
  SensitiveAsk,
  UnrecognisedModel,
} from './model';
import { transferAt } from '../shared/transferFixtures';

const FOUND_APPS: readonly FoundApp[] = [
  { ...APPS.git, what: 'Name, email, aliases, global ignore', files: 2, included: true },
  { ...APPS.ssh, what: 'Server list and shortcuts', files: 3, included: true },
  { ...APPS.gpg, what: 'Public keys and trust settings', files: 4, included: true },
  { ...APPS.shell, what: 'History, aliases and prompt theme', files: 15, included: true },
  { ...APPS.vscode, what: 'Settings, keybindings, snippets', files: 388, included: true },
];

export const ASKS: readonly SensitiveAsk[] = [
  {
    key: 'ssh/keys',
    title: 'Your SSH keys',
    why: 'They log you in to servers and GitHub. Anyone with a copy can do the same, as you.',
    kind: 'secret',
    answer: 'leave',
  },
  {
    key: 'gpg/secret-keys',
    title: 'Your GnuPG secret keys',
    why: 'They sign your commits and open mail sent to you. Anyone with a copy can do both.',
    kind: 'secret',
    answer: 'leave',
  },
  {
    key: 'shell/rc',
    title: 'Shell startup files',
    why: 'They run every time you open a terminal. Bring them if you wrote or checked them yourself.',
    kind: 'runs-code',
    answer: 'leave',
  },
];

export const foundScanning: FoundModel = {
  status: 'scanning',
  foundSoFar: ['Git', 'SSH', 'GnuPG'],
  progress: 0.45,
};
export const foundReady: FoundModel = {
  status: 'found',
  apps: FOUND_APPS,
  asks: ASKS,
  issues: [],
};
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

export const liveReviewing: LiveModel = {
  status: 'reviewing',
  here: SENDER,
  peer: RECEIVER,
  replaced: 3,
};
export const liveMoving: LiveModel = { status: 'transfer', transfer: transferAt('moving', 'send') };
export const liveReconnecting: LiveModel = {
  status: 'transfer',
  transfer: transferAt('reconnecting', 'send'),
};

export const delivered: DeliveredModel = { peer: RECEIVER, apps: 5, bytes: 1.2 * GB };
