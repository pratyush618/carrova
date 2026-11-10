import { APPS, GB, RECEIVER, SENDER } from '../shared/fixtures';
import { transferAt } from '../shared/transferFixtures';
import type { MoveRoute } from '../shared/model';
import type {
  ArrivedModel,
  CodeEntryModel,
  DecisionModel,
  MappingModel,
  PluginConsentModel,
  ReceiveProgressModel,
  ReviewDetailsModel,
  ReviewModel,
} from './model';

const WORDS = [
  'zebra',
  'zesty',
  'zippy',
  'zone',
  'lemon',
  'lemur',
  'level',
  'orbit',
  'maple',
  'acid',
  'acorn',
  'bagel',
];

export const codeTyping: CodeEntryModel = {
  status: 'typing',
  value: '7-zebra-lem',
  peer: null,
  words: WORDS,
  address: '',
};
export const codeConnecting: CodeEntryModel = {
  status: 'connecting',
  here: RECEIVER,
  peer: SENDER,
  code: ['7', 'zebra', 'lemon'],
};
export const codeNotFound: CodeEntryModel = {
  status: 'not-found',
  value: '7-zebra-lemon',
  peer: null,
  words: WORDS,
  address: '',
};
export const codeMismatch: CodeEntryModel = {
  status: 'mismatch',
  value: '',
  peer: SENDER,
  words: WORDS,
  address: '',
};

/** The fictional move: ezio-desktop (Linux) to this computer. */
export const ROUTE: MoveRoute = {
  from: { name: SENDER, os: 'linux' },
  to: { name: RECEIVER, os: 'linux' },
};

/** The same move from a Mac, so items cross systems. */
const CROSS_ROUTE: MoveRoute = { ...ROUTE, from: { name: SENDER, os: 'mac_os' } };

const decisions = (answered: boolean): DecisionModel[] => [
  {
    kind: 'secret',
    key: 'ssh/keys',
    title: 'Your SSH keys',
    why: 'They log you in to servers and GitHub. Anyone with a copy can do the same, as you.',
    answer: answered ? 'bring' : null,
  },
  {
    kind: 'secret',
    key: 'gpg/secret-keys',
    title: 'Your GnuPG secret keys',
    why: "They sign your commits and open mail sent to you. GnuPG is stopped for a moment while they're placed.",
    answer: answered ? 'leave' : null,
  },
  { kind: 'app-running', key: 'vscode', app: 'VS Code', answer: answered ? 'closed' : null },
];

const review = (answered: boolean): ReviewModel => ({
  counts: { added: 412, replaced: 3, identical: 28, left: 1 },
  decisions: decisions(answered),
  space: { needed: 1.2 * GB, free: 112 * GB, total: 256 * GB, short: false, ignored: false },
  mapped: null,
  rule: 'replace',
  retention: { keep_for: { days: 30 } },
  blocked: !answered,
  rail: {
    route: ROUTE,
    apps: { moving: 5, offered: 6 },
    bytes: 1.2 * GB,
    keys: answered ? ['Your SSH keys'] : null,
  },
  busy: false,
});

export const reviewDecide = review(false);
export const reviewReady = review(true);

/** From a Mac: a Linux-only item to answer, a plugin to allow, and paths that change place. */
export const reviewCrossOs: ReviewModel = {
  ...reviewDecide,
  decisions: [
    {
      kind: 'cross-os',
      key: 'gnome-terminal/profile',
      title: 'GNOME Terminal profile',
      from: 'mac_os',
      to: 'linux',
      answer: null,
    },
    {
      kind: 'plugin',
      key: 'path-rewrite',
      description: 'Rewrites home folder paths in the vault list for this computer.',
      items: ['Vault list'],
      answer: null,
    },
  ],
  mapped: { from: 'mac_os', to: 'linux', apps: 2 },
  rail: { ...reviewDecide.rail, route: CROSS_ROUTE },
};

export const mapping: MappingModel = {
  from: 'mac_os',
  to: 'linux',
  moved: [
    {
      key: 'vscode/settings#0',
      glyph: '{ }',
      title: 'VS Code settings',
      from: { root: 'user_data', rel: 'Code/User', abs: null },
      to: { root: 'user_config', rel: 'Code/User', abs: '/home/ezio/.config/Code/User' },
    },
  ],
  same: [
    {
      key: 'git/config#0',
      glyph: 'git',
      title: 'Git',
      from: { root: 'home', rel: '.gitconfig', abs: null },
      to: { root: 'home', rel: '.gitconfig', abs: '/home/ezio/.gitconfig' },
    },
  ],
};

export const pluginConsent: PluginConsentModel = {
  plugin: 'path-rewrite',
  version: '1.0.0',
  description: 'Rewrites home folder paths in the vault list for this computer.',
  permissions: [
    'It sees the path of your home folder on this computer.',
    'It cannot use the network.',
  ],
  items: ['Vault list'],
};

export const reviewDetails: ReviewDetailsModel = {
  groups: [
    {
      ...APPS.ssh,
      rule: 'replace',
      clashes: [{ path: '~/.ssh/config', display: '~/.ssh/config', rule: null }],
      added: 2,
      canRemap: false,
      remapped: false,
    },
    {
      ...APPS.git,
      rule: 'replace',
      clashes: [{ path: '~/.gitconfig', display: '~/.gitconfig', rule: null }],
      added: 1,
      canRemap: false,
      remapped: false,
    },
    {
      ...APPS.vscode,
      rule: 'replace',
      clashes: [
        { path: 'Code/User/settings.json', display: 'Code/User/settings.json', rule: null },
      ],
      added: 386,
      canRemap: true,
      remapped: false,
    },
  ],
  choices: {
    keepBothSuffix: 'carrova-{date}',
    retention: 'until_dismissed',
    importTrust: false,
    keepGroupWrite: false,
  },
};

export const progressAt = (phase: Parameters<typeof transferAt>[0]): ReceiveProgressModel => ({
  status: 'transfer',
  transfer: transferAt(phase, 'receive'),
  resuming: false,
  resumeError: null,
});

export const progressFailed: ReceiveProgressModel = {
  status: 'failed',
  error: {
    code: 'internal',
    message: "Couldn't write VS Code's settings: permission denied.",
    hint: null,
  },
};

const arrived = (partial: boolean): ArrivedModel => ({
  route: ROUTE,
  moved: partial
    ? ['Git', 'SSH', 'Bash and Zsh', 'VS Code']
    : ['Git', 'SSH', 'GnuPG', 'Bash and Zsh', 'VS Code', 'tmux'],
  appsTotal: 6,
  files: 415,
  bytes: 1.2 * GB,
  replaced: 3,
  failures: partial
    ? [{ what: 'GnuPG trust settings', reason: 'the folder is locked by another program' }]
    : [],
  toInstall: ['VS Code', 'tmux'],
});

export const arrivedAll = arrived(false);
export const arrivedPartial = arrived(true);
