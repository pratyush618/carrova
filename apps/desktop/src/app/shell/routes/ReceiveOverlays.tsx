import { detailsModel } from '../../../flows/receive/reviewViews';
import { current } from '../../../flows/remote';
import { RemapDialog } from '../../../screens/receive/RemapDialog';
import { ReviewDetailsSheet } from '../../../screens/receive/ReviewDetailsSheet';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

export function ReviewDetailsOverlay() {
  const { receive } = useControllers();
  const nav = useNav();
  const plan = current(receive.state.review);
  if (plan === null) return null;
  return (
    <ReviewDetailsSheet
      model={detailsModel(plan, receive.state.options)}
      on={{
        onRule: receive.setRule,
        onFileRule: (_item, path, rule) => {
          receive.setFileRule(path, rule);
        },
        onRemap: (item) => {
          nav.open({ kind: 'override', reason: 'elsewhere', subject: item });
        },
        onChoices: receive.setChoices,
        onDone: nav.close,
      }}
    />
  );
}

export function RemapOverlay({ item }: { readonly item: string }) {
  const { receive } = useControllers();
  const nav = useNav();
  const label = current(receive.state.review)?.offered.find((o) => o.key === item)?.label ?? item;
  return (
    <RemapDialog
      model={{ item: label, current: receive.state.options.remaps[item] ?? null }}
      on={{
        onCancel: nav.close,
        onSave: (remap) => {
          receive.setRemap(item, remap);
          nav.close();
        },
        onReset: () => {
          receive.setRemap(item, null);
          nav.close();
        },
      }}
    />
  );
}
