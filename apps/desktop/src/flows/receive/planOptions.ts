import type { MoveDefaults, PlanOptions } from '../../engine/types';
import type { PlanEdit } from '../../bindings/PlanEdit';
import { ENGINE_DEFAULTS } from '../../bindings/constants';
import type { DecisionAnswer, ImportChoices } from '../../screens/receive/model';

/**
 * The engine's suggested options (`ENGINE_DEFAULTS.plan_options`: every offered
 * item the receiver doesn't treat as sensitive, back up and replace on clashes,
 * nothing overridden) with the user's own suggested defaults (Settings) on top.
 */
export function suggestedPlanOptions(defaults: MoveDefaults): PlanOptions {
  return {
    ...ENGINE_DEFAULTS.plan_options,
    default_rule: defaults.conflict_rule,
    retention: defaults.retention,
  };
}

/** A review answer as plan edits: secrets confirm or drop (`key` = item); open apps skip or stay (`key` = app). */
export function answerEdits(key: string, answer: DecisionAnswer): PlanEdit[] {
  switch (answer) {
    case 'bring':
    case 'leave':
      return [{ kind: 'answer_sensitive', item: key, bring: answer === 'bring' }];
    case 'skip':
    case 'closed':
      return [{ kind: 'skip_app', app: key, skip: answer === 'skip' }];
  }
}

/** Import-wide choices from the Every change sheet as plan edits (an empty suffix keeps the suggestion). */
export function choiceEdits(patch: Partial<ImportChoices>): PlanEdit[] {
  const edits: PlanEdit[] = [];
  if (patch.keepBothSuffix !== undefined) {
    edits.push({ kind: 'keep_both_suffix', suffix: patch.keepBothSuffix });
  }
  if (patch.retention !== undefined) {
    edits.push({ kind: 'retention', retention: patch.retention });
  }
  if (patch.importTrust !== undefined) {
    edits.push({ kind: 'import_trust', allow: patch.importTrust });
  }
  if (patch.keepGroupWrite !== undefined) {
    edits.push({ kind: 'keep_group_write', keep: patch.keepGroupWrite });
  }
  return edits;
}
