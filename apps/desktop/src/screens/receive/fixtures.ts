import { APPS, GB, RECEIVER, SENDER } from '../shared/fixtures';
import { transferAt } from '../shared/transferFixtures';
import type {
  ArrivedModel,
  CodeEntryModel,
  DecisionModel,
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
  counts: { added: 412, replaced: 3, identical: 28 },
  undoDays: 30,
  decisions: decisions(answered),
  space: { needed: 1.2 * GB, free: 112 * GB, short: false, ignored: false },
  blockers: [],
  busy: false,
});

export const reviewDecide = review(false);
export const reviewReady = review(true);

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
  peer: SENDER,
  apps: partial ? 4 : 5,
  appsTotal: 5,
  bytes: 1.2 * GB,
  replaced: 3,
  failures: partial
    ? [{ what: 'GnuPG trust settings', reason: 'the folder is locked by another program' }]
    : [],
  tasks: [
    { kind: 'reinstall', app: 'VS Code', count: 12, command: 'code --install-extension …' },
    { kind: 'new-terminal' },
    { kind: 'sign-in', app: 'GitHub CLI' },
  ],
});

export const arrivedAll = arrived(false);
export const arrivedPartial = arrived(true);
