/**
 * Recorded bridge results the fake replays (see `../fixtures`, written by
 * `just ipc-fixtures` from real bridge runs), plus the LAN-only data a
 * recording over fake homes cannot produce.
 */
import type { CodeView } from '../../bindings/CodeView';
import type { Inventory } from '../../bindings/Inventory';
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
  return { code, fallback: `${code}@192.168.1.20:47822`, renewed };
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
