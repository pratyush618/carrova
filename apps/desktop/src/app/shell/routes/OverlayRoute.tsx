import { FileChooseDialog } from '../../../screens/start/FileChooseDialog';
import type { Overlay } from '../../nav/navMachine';
import { useNav } from '../nav';
import {
  AboutOverlay,
  ForgetOverlay,
  ReportOverlay,
  UndoConfirmOverlay,
  UpdateOverlay,
} from './HubOverlays';
import { OverrideRoute } from './OverrideRoute';
import {
  MappingOverlay,
  PluginConsentOverlay,
  RemapOverlay,
  ReviewDetailsOverlay,
} from './ReceiveOverlays';
import { CustomizeOverlay, UnrecognisedOverlay } from './SendOverlays';
import { useStartFlows } from './useStartFlows';

function FileChooseOverlay() {
  const nav = useNav();
  const start = useStartFlows();
  return <FileChooseDialog on={{ onSave: start.save, onOpen: start.open, onCancel: nav.close }} />;
}

/** One overlay (sheet or dialog). Every one closes with Esc via its own handler. */
export function OverlayRoute({ overlay }: { readonly overlay: Overlay }) {
  switch (overlay.kind) {
    case 'file-choose':
      return <FileChooseOverlay />;
    case 'customize':
      return <CustomizeOverlay />;
    case 'unrecognised':
      return <UnrecognisedOverlay />;
    case 'review-details':
      return <ReviewDetailsOverlay />;
    case 'mappings':
      return <MappingOverlay />;
    case 'plugin-consent':
      return <PluginConsentOverlay plugin={overlay.plugin} />;
    case 'remap':
      return <RemapOverlay item={overlay.item} />;
    case 'undo-confirm':
      return <UndoConfirmOverlay />;
    case 'forget':
      return <ForgetOverlay id={overlay.id} />;
    case 'update':
      return <UpdateOverlay />;
    case 'report':
      return <ReportOverlay />;
    case 'about':
      return <AboutOverlay />;
    case 'override':
      return <OverrideRoute reason={overlay.reason} subject={overlay.subject} />;
  }
}
