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
  ManualPick,
  ModeSwitchModel,
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
    why: 'login',
    paths: ['~/.ssh'],
    keys: [
      {
        path: '~/.ssh/id_ed25519',
        kind: 'ed25519',
        fingerprint: 'SHA256:qX3fT0dVb8yZ1kJm4pR7sWc2eHn5uLa9gQ6iO3xY9LmA',
        comment: 'ana@ana-laptop',
      },
      {
        path: '~/.ssh/id_ed25519_work',
        kind: 'ed25519',
        fingerprint: 'SHA256:7PkcR2wN8vB4tE6yH1jM9sL3qF5dA0zX2gK7nU4oUe2w',
        comment: null,
      },
    ],
    bytes: 6 * KB,
    brought: false,
  },
  {
    key: 'gpg/secret-keys',
    app: APPS.gpg,
    label: 'GnuPG secret keys',
    why: 'signing',
    paths: ['~/.gnupg/private-keys-v1.d'],
    keys: [],
    bytes: 12 * KB,
    brought: false,
  },
  {
    key: 'git/credentials',
    app: APPS.git,
    label: 'Stored Git credentials',
    why: 'token',
    paths: ['~/.git-credentials'],
    keys: [],
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
  plugins: [],
} satisfies FoundModel;
export const foundNothing: FoundModel = { status: 'nothing', knownApps: 15 };

/** The Auto/Manual switch in each mode (Auto has ticked 4 apps; Manual holds 2 picks). */
export const MODE_AUTO: ModeSwitchModel = { manual: false, picks: 2, apps: 4, disabled: false };
export const MODE_MANUAL: ModeSwitchModel = { ...MODE_AUTO, manual: true };

export const PICKS: readonly ManualPick[] = [
  {
    key: 'raw:home/projects/thesis',
    path: '~/projects/thesis',
    kind: 'dir',
    bytes: 1.2 * GB,
    files: 2_314,
    sensitive: false,
    included: true,
  },
  {
    key: 'raw:home/work/api/.env',
    path: '~/work/api/.env',
    kind: 'file',
    bytes: 2 * KB,
    files: 1,
    sensitive: true,
    included: false,
  },
];

const MANUAL_FACTS: MoveFacts = {
  ...FACTS,
  apps: 0,
  totalApps: 0,
  bytes: 1.2 * GB,
  picks: 1,
  secrets: 0,
};

export const manualEmpty = {
  status: 'manual',
  picks: [],
  measuring: [],
  rejected: [],
  absorbed: [],
  facts: { ...MANUAL_FACTS, bytes: 0, picks: 0 },
  issues: [],
} satisfies FoundModel;
export const manualPicks = {
  ...manualEmpty,
  picks: PICKS,
  facts: MANUAL_FACTS,
} satisfies FoundModel;
export const manualMeasuring = {
  ...manualPicks,
  measuring: ['~/Music/sessions'],
} satisfies FoundModel;
export const manualRejected = {
  ...manualPicks,
  rejected: [
    { path: '/etc/hosts', reason: { kind: 'outside_roots' } },
    { path: '~', reason: { kind: 'is_root', root: 'home' } },
  ],
  absorbed: [{ path: '~/projects/thesis/notes.md', into: '~/projects/thesis' }],
} satisfies FoundModel;
export const manualSensitive = {
  ...manualPicks,
  picks: PICKS.map((p) => (p.sensitive ? { ...p, included: true } : p)),
  facts: { ...MANUAL_FACTS, picks: 2, secrets: 1 },
} satisfies FoundModel;

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

/** The code's deadline: 4:52 from when the fixtures load (the gallery counts down from there). */
const EXPIRES_AT = Date.now() + 292_000;

export const pairingWaiting: PairingModel = {
  status: 'waiting',
  code: CODE,
  address: ADDRESS,
  expiresAt: EXPIRES_AT,
};
export const pairingCantConnect: PairingModel = { ...pairingWaiting, status: 'cant-connect' };
export const pairingRenewed: PairingModel = {
  status: 'renewed',
  code: ['7', 'acorn', 'maple'],
  address: '7-acorn-maple@192.168.1.20:47822',
  expiresAt: EXPIRES_AT,
};
export const pairingExpired: PairingModel = { ...pairingRenewed, status: 'expired' };
export const pairingChecking: PairingModel = { status: 'checking' };
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
