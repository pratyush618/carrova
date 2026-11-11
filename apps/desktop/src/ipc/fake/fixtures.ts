/**
 * Recorded bridge results the fake replays (see `../fixtures`, written by
 * `just ipc-fixtures` from real bridge runs), plus the LAN-only data a
 * recording over fake homes cannot produce.
 */
import type { CodeView } from '../../bindings/CodeView';
import type { Inventory } from '../../bindings/Inventory';
import type { PlannedPlugin } from '../../bindings/PlannedPlugin';
import type { PlanReview } from '../../bindings/PlanReview';
import type { PlanSummary } from '../../bindings/PlanSummary';
import type { SessionId } from '../../bindings/SessionId';
import { plan } from '../fixtures/plan';
export { planUnconfirmed } from '../fixtures/plan-unconfirmed';
import { scan } from '../fixtures/scan';

export { archiveSummary } from '../fixtures/archive-summary';
export { catalog } from '../fixtures/catalog';
export { checklist } from '../fixtures/checklist';
export { exportEvents } from '../fixtures/export-events';
export { importEvents } from '../fixtures/import-events';
export { recipesStatus } from '../fixtures/recipes-status';
export { rollback } from '../fixtures/rollback';
export { rollbackPreview } from '../fixtures/rollback-preview';
export { sessions } from '../fixtures/sessions';
export { plan, scan };

/** The export session of the recording. */
export const exportSession: SessionId = scan.session;

/** The import session of the recording. */
export const importSession: SessionId = plan.plan.session;

/** The code a fake send shows (first, then after a wrong attempt). */
export function codeView(renewed: boolean): CodeView {
  const code = renewed ? '4-violet-harbor' : '7-acid-lemon';
  return {
    code,
    fallback: `${code}@192.168.1.20:47822`,
    renewed,
    expired: false,
    expires_in_secs: 300,
  };
}

/** The fake receiver's machine. */
export const inventory: Inventory = {
  os: 'linux',
  carrova_version: '0.1.0',
  apps: [
    { id: 'git', version: null },
    { id: 'neovim', version: null },
  ],
  free_space: 120_000_000_000,
  device_name: 'ana-desktop',
};

/** The fake receiver's plan, as the sender sees it. */
export const planSummary: PlanSummary = {
  items: plan.offered
    .filter((item) => item.picked)
    .map((item) => ({
      item: item.key,
      counts: { create: 1, replace: 0, keep_both: 0, skip: 0, identical: 0 },
      conflicts: [],
    })),
};

/** Where a fake receiver connects from / to. */
export const peerAddress = '192.168.1.20:47822';

/**
 * The `plugin` scenario's plugin, over the Starship config: the sender is
 * asked to allow its export, the receiver its transform.
 */
export const PATH_REWRITE = {
  plugin: 'path-rewrite',
  version: '1.0.0',
  description: 'Rewrites home folder paths in the config for this computer.',
  permissions: [
    'It sees the path of your home folder on this computer.',
    'It cannot use the network.',
  ],
  items: ['starship/config'],
} satisfies PlannedPlugin;

/** One Manual pick of the `picks` scenario: one file or folder under `home`. */
interface FakePick {
  readonly rel: string;
  readonly kind: 'file' | 'dir';
  readonly size: number;
  /** Why the receiver treats it as sensitive (the engine's words). */
  readonly reason: string;
  /** Only "unrecognised data": Accept all ticks it. */
  readonly safe: boolean;
}

const UNRECOGNISED = 'unrecognised data from the archive';

const PICKS: readonly FakePick[] = [
  { rel: 'projects/site', kind: 'dir', size: 0, reason: UNRECOGNISED, safe: true },
  { rel: 'notes.txt', kind: 'file', size: 3_100, reason: UNRECOGNISED, safe: true },
  {
    rel: '.ssh/id_ed25519',
    kind: 'file',
    size: 411,
    reason: `marked sensitive in the archive; ${UNRECOGNISED}`,
    safe: false,
  },
];

/**
 * The `picks` scenario's plan: the recording plus three Manual picks, planned
 * with every sensitive item confirmed like the recording (`planUnder` re-plans
 * it under the user's options). Picks arrive unticked; two are safe.
 */
export const planWithPicks: PlanReview = (() => {
  const keyOf = (p: FakePick) => `raw:home/${p.rel}`;
  const actions = PICKS.map((p) => ({
    abs: `/home/ana/${p.rel}`,
    action: { action: 'create' as const },
    entry_index: 0,
    item: keyOf(p),
    kind: p.kind,
    link_target: null,
    mode: p.kind === 'dir' ? 0o755 : 0o600,
    mtime_ns: 1.79438244e18,
    rel: p.rel,
    root: 'home' as const,
    size: p.size,
  }));
  return {
    ...plan,
    offered: [
      ...plan.offered,
      ...PICKS.map((p) => ({
        key: keyOf(p),
        label: `~/${p.rel}`,
        picked: true,
        safe_pick: p.safe,
        sensitive: { confirmed: true, reason: p.reason },
      })),
    ],
    plan: {
      ...plan.plan,
      actions: [...plan.plan.actions, ...actions],
      safe_picks: PICKS.filter((p) => p.safe).map(keyOf),
      sensitive: {
        ...plan.plan.sensitive,
        ...Object.fromEntries(PICKS.map((p) => [keyOf(p), p.reason])),
      },
    },
    view: {
      ...plan.view,
      items: [
        ...plan.view.items,
        ...actions.map((a) => ({
          key: a.item,
          counts: {
            actions: { create: 1, identical: 0, keep_both: 0, replace: 0, skip: 0 },
            bytes: a.size,
          },
          lines: [{ line: 'action' as const, ...a }],
        })),
      ],
    },
  };
})();
