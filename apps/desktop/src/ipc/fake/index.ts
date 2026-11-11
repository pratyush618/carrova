/**
 * An in-memory `CarrovaPort` that acts out realistic flows from recorded
 * bridge fixtures, for tests, Playwright and the gallery. Timing is
 * controlled by `stepMs` (and an injectable `sleep`); the flow by a
 * `Scenario`. Conventions: the archive passphrase `wrong` is rejected, export
 * refuses passphrases under 10 characters (see `strengthOf`), and codes must
 * look like `7-acid-lemon`.
 */
import type { About } from '../../bindings/About';
import type { FileManagerStatus } from '../../bindings/FileManagerStatus';
import type { Notice } from '../../bindings/Notice';
import type { ReportConsent } from '../../bindings/ReportConsent';
import type { Settings } from '../../bindings/Settings';
import type { CarrovaPort } from '../port';
import { bridgeError, raise } from '../reject';
import * as fx from './fixtures';
import { FakeReceiver } from './receive';
import { later, realSleep, Runtime, type Sleep } from './runtime';
import type { Scenario } from './scenario';
import { FakeSender } from './send';

export { WRONG_PASSPHRASE } from './receive';
export { SCENARIOS, scenarioFromSearch, type Scenario } from './scenario';

/** How the fake behaves. */
export interface FakeOptions {
  /** Flow to act out (default `happy`). */
  scenario?: Scenario;
  /** Milliseconds per step of a long task (default 150; 0 in unit tests). */
  stepMs?: number;
  /** How to wait (default: real timers). */
  sleep?: Sleep;
}

const DEFAULT_SETTINGS: Settings = {
  auto_check_recipes: true,
  auto_check_app: true,
  theme: null,
  reduce_motion: null,
  move_defaults: { conflict_rule: 'replace', retention: 'until_dismissed', include_caches: false },
  start_in_manual: false,
  file_manager_asked: false,
  background: { keep_running: true, start_at_login: false, lean: true },
  alerts: {
    system: true,
    wake_screen: true,
    names_on_lock_screen: false,
    paused_until: null,
    requests: 'popup_and_bell',
    moves: 'popup_and_bell',
    nearby: 'bell_only',
    updates: 'bell_only',
  },
};

/** This (fake) machine, for the About sheet. */
const ABOUT: About = {
  app_version: '0.1.0',
  engine_version: '0.1.0',
  device_name: 'ana-laptop',
  os: 'linux',
};

/** The bell's notices at the start: a request, a finished move, an old one. */
function seedNotices(nowSeconds: number): Notice[] {
  return [
    {
      id: 3,
      kind: 'send_request',
      at: nowSeconds - 120,
      read: false,
      peer: "Ana's laptop",
      summary: 'wants to send you 3 apps · 1.2 GB',
    },
    {
      id: 2,
      kind: 'move_finished',
      at: nowSeconds - 1200,
      read: false,
      peer: null,
      summary: 'The move finished.',
    },
    {
      id: 1,
      kind: 'update_ready',
      at: nowSeconds - 3 * 86_400,
      read: true,
      peer: null,
      summary: 'Carrova 0.3.0 is ready to install.',
    },
  ];
}

/** What the `forwarded` scenario's file manager sent (paths on the fake disk). */
const FORWARDED = ['/home/ana/projects/site', '/home/ana/notes.txt'];

/** The recipe list `recipes-update` offers. */
const NEWER_RECIPES = 2;
const UP_TO_DATE = { kind: 'up_to_date' } as const;

/** A fake port acting out `options.scenario`. */
export function createFakePort(options: FakeOptions = {}): CarrovaPort {
  const scenario = options.scenario ?? 'happy';
  const runtime = new Runtime(options.stepMs ?? 150, options.sleep ?? realSleep);
  const sender = new FakeSender(runtime, scenario);
  const receiver = new FakeReceiver(runtime, scenario);
  let settings = DEFAULT_SETTINGS;
  let consent: ReportConsent = 'unset';
  let notices = seedNotices(Math.floor(Date.now() / 1000));
  // shortcut: an install doesn't change `recipesStatus`; make it stateful if a test needs the new version shown.
  let recipesInstalled = false;
  const newerRecipes = () => scenario === 'recipes-update' && !recipesInstalled;
  const noReport = () => later(() => raise(bridgeError('session', 'no such error report')));
  let fileManager: FileManagerStatus = { supported: true, installed: false, skipped: [] };
  /** Paths "sent from the file manager", handed over once. */
  let forwarded = scenario === 'forwarded' ? [...FORWARDED] : [];

  return {
    subscribe: (listener) => later(() => runtime.subscribe(listener)),

    scan: () => sender.scan(),
    startManual: () => sender.startManual(),
    scanApps: (session) => sender.scanApps(session),
    pick: (session, selection, paths) => sender.pick(session, selection, paths),
    takeSendRequest: () =>
      later(() => {
        const paths = forwarded;
        forwarded = [];
        return paths;
      }),
    suggest: (session, preset) => sender.suggest(session, preset),
    select: (session, selection, edits) => sender.select(session, selection, edits),
    passphraseStrength: (passphrase) => sender.passphraseStrength(passphrase),
    exportArchive: (session, _path, passphrase) => sender.exportArchive(session, passphrase),
    runSend: (session) => sender.runSend(session),
    answerPairing: (session, offerNewCode) =>
      later(() => {
        runtime.answer(session, offerNewCode);
      }),

    openArchive: (_path, passphrase) => receiver.openArchive(passphrase),
    connect: (code) => receiver.connect(code),
    cancelConnect: () =>
      later(() => {
        receiver.cancelConnect();
      }),
    browseNearby: () => later(() => []),
    plan: (session, options) => receiver.plan(session, options),
    editPlan: (session, edits) => receiver.editPlan(session, edits),
    planOptions: (session) => receiver.planOptions(session),
    approve: (session) => receiver.approve(session),
    apply: (session) => receiver.apply(session),
    endReview: (session) => receiver.endReview(session),
    setTransferPaused: (session, paused) => receiver.setTransferPaused(session, paused),
    ignoreFreeSpace: (session) => receiver.ignoreFreeSpace(session),
    retryConnection: (session) => receiver.retryConnection(session),

    cancel: (session) =>
      later(() => {
        runtime.cancel(session);
      }),
    resume: (session) =>
      later(() => {
        runtime.open(session);
        return { session };
      }),
    sessions: () => later(() => fx.sessions),
    rollback: (session) =>
      later(() => {
        const target = session ?? fx.importSession;
        for (const path of fx.rollback.restored) {
          runtime.engine(target, { event: 'rollback_step', target: path, outcome: 'restored' });
        }
        return fx.rollback;
      }),
    rollbackPreview: () => later(() => fx.rollbackPreview),
    dismiss: (session) =>
      later(() => {
        runtime.forget(session);
      }),

    catalog: () =>
      later(() =>
        scenario === 'no-recipes'
          ? raise(bridgeError('recipes', 'no recipes available'))
          : fx.catalog,
      ),
    recipesStatus: () => later(() => fx.recipesStatus),
    recipesCheck: () =>
      later(() =>
        newerRecipes()
          ? ({ kind: 'available', version: NEWER_RECIPES } as const)
          : ({ kind: 'up_to_date', version: NEWER_RECIPES - 1 } as const),
      ),
    recipesUpdate: () =>
      later(() => {
        if (!newerRecipes()) return UP_TO_DATE;
        recipesInstalled = true;
        return { kind: 'updated', version: NEWER_RECIPES } as const;
      }),
    recipesReset: () => later(() => fx.recipesStatus),

    about: () => later(() => ABOUT),

    settings: () => later(() => settings),
    saveSettings: (next) =>
      later(() => {
        settings = next;
      }),
    fileManagerStatus: () => later(() => fileManager),
    setFileManagerEntries: (on) =>
      later(() => {
        fileManager = { ...fileManager, installed: on };
        return fileManager;
      }),
    setBackground: (background) =>
      later(() => {
        settings = { ...settings, background };
      }),

    notices: () => later(() => notices),
    markNoticesRead: (ids) =>
      later(() => {
        notices = notices.map((n) =>
          ids === 'all' || ids.includes(n.id) ? { ...n, read: true } : n,
        );
      }),

    checkUpdate: () => later(() => ({ status: 'up_to_date' }) as const),
    installUpdate: () => later(() => undefined),
    restartApp: () => later(() => undefined),

    errorReportConsent: () => later(() => consent),
    setErrorReportConsent: (next) =>
      later(() => {
        consent = next;
      }),
    errorReports: () => later(() => []),
    errorReport: noReport,
    deleteErrorReport: noReport,
    clearErrorReports: () => later(() => undefined),
    errorReportIssue: noReport,
  };
}
