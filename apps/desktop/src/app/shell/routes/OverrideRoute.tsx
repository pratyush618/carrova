import { appName, itemLabel } from '../../../flows/receive/reviewViews';
import { current } from '../../../flows/remote';
import { OverrideDialog } from '../../../screens/override/OverrideDialog';
import type { OverrideModel } from '../../../screens/override/model';
import { useControllers, type Controllers } from '../../controllers/Controllers';
import type { OverrideKind } from '../../nav/navMachine';
import { useNav } from '../nav';

/** What the warning says, from live data (the item's own reason, the app that is open). */
function overrideModel(
  reason: OverrideKind,
  subject: string | null,
  c: Controllers,
): OverrideModel | null {
  switch (reason) {
    case 'unmovable': {
      const key = subject ?? '';
      const choice = c.send.state.selection?.items[key];
      const scan = current(c.send.state.scan);
      const label =
        scan?.report.apps
          .flatMap((a) => a.items.map((i) => ({ key: `${a.id}/${i.id}`, label: i.label })))
          .find((i) => i.key === key)?.label ?? key;
      return { kind: 'unmovable', item: label, reason: choice?.unmovable ?? '', count: 1 };
    }
    case 'running': {
      const review = current(c.receive.state.review);
      const id = subject ?? '';
      return { kind: 'running', app: review === null ? id : appName(review, id) };
    }
    case 'space':
      return { kind: 'space' };
    case 'cross_os': {
      const key = subject ?? '';
      const review = current(c.receive.state.review);
      const asked = c.receive.state.asked.crossOs[key];
      if (asked === undefined) return null;
      return { kind: 'cross_os', item: review === null ? key : itemLabel(review, key), ...asked };
    }
    case 'elsewhere':
      return { kind: 'elsewhere' };
  }
}

/** Every override goes through the one warning dialog; the safe choice changes nothing. */
export function OverrideRoute({
  reason,
  subject,
}: {
  readonly reason: OverrideKind;
  readonly subject: string | null;
}) {
  const controllers = useControllers();
  const { send, receive } = controllers;
  const nav = useNav();
  const confirm = () => {
    nav.close();
    switch (reason) {
      case 'unmovable':
        if (subject !== null) send.edit({ kind: 'force', key: subject });
        return;
      case 'running':
        receive.override('running', subject);
        return;
      case 'cross_os':
        if (subject !== null) receive.allowCrossOs(subject);
        return;
      case 'space':
        // During a transfer the check is lifted live; before it, in the plan.
        if (receive.state.apply.status === 'loading') receive.continueDespiteSpace();
        else receive.override('space', null);
        return;
      case 'elsewhere':
        if (subject !== null) nav.open({ kind: 'remap', item: subject });
        return;
    }
  };
  const model = overrideModel(reason, subject, controllers);
  if (model === null) return null;
  return <OverrideDialog model={model} on={{ onKeepSafe: nav.close, onOverride: confirm }} />;
}
