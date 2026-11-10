import { mappingModel } from '../../../flows/receive/mappingViews';
import { detailsModel, pluginConsentModel } from '../../../flows/receive/reviewViews';
import { current } from '../../../flows/remote';
import { MappingSheet } from '../../../screens/receive/MappingSheet';
import { PluginConsentDialog } from '../../../screens/receive/PluginConsentDialog';
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

/** Where cross-system items land on this computer. */
export function MappingOverlay() {
  const { receive, settings } = useControllers();
  const nav = useNav();
  const plan = current(receive.state.review);
  const here = settings.about?.os;
  const model = plan === null || here === undefined ? null : mappingModel(plan, here);
  if (model === null) return null;
  return <MappingSheet model={model} onClose={nav.close} />;
}

/** A plugin's permissions, asked on this computer: allow for this move or leave its items. */
export function PluginConsentOverlay({ plugin }: { readonly plugin: string }) {
  const { receive } = useControllers();
  const nav = useNav();
  const plan = current(receive.state.review);
  const model = plan === null ? null : pluginConsentModel(plan, receive.state.asked, plugin);
  if (model === null) return null;
  return (
    <PluginConsentDialog
      model={model}
      on={{
        onAllow: () => {
          receive.allowPlugin(plugin);
          nav.close();
        },
        onDecline: () => {
          receive.leavePlugin(plugin);
          nav.close();
        },
        onClose: nav.close,
      }}
    />
  );
}
