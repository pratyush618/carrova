import type { NotifyAppRow } from '../../bindings/NotifyAppRow';
import type { PermStep } from '../../bindings/PermStep';
import type { PhonePerm } from '../../bindings/PhonePerm';
import type { Phase2Feature } from '../../screens/devices/model';

/** The Android permissions each phone feature needs, asked in this order (spec, Android bridge). */
export const FEATURE_PERMS: Readonly<Partial<Record<Phase2Feature, readonly PhonePerm[]>>> = {
  notifications: ['notification_access'],
  messages: ['sms', 'contacts'],
  calls: ['calls', 'contacts'],
};

/** The first of `perms` not granted yet, with the step it needs; `null` once all are. */
export function pendingPerm(
  perms: readonly PhonePerm[],
  steps: readonly PermStep[],
): { readonly perm: PhonePerm; readonly step: Exclude<PermStep, 'ok'> } | null {
  for (const [i, perm] of perms.entries()) {
    const step = steps[i];
    if (step !== undefined && step !== 'ok') return { perm, step };
  }
  return null;
}

/** The apps whose name or package id holds `query` (any case). */
export const appsMatching = (
  apps: readonly NotifyAppRow[],
  query: string,
): readonly NotifyAppRow[] => {
  const q = query.trim().toLowerCase();
  return apps.filter((a) => a.name.toLowerCase().includes(q) || a.id.toLowerCase().includes(q));
};
