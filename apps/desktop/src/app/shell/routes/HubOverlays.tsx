import { historyEntry } from '../../../flows/history/historyViews';
import { knownDeviceLine } from '../../../flows/nearby/nearbyViews';
import {
  ForgetDevicesDialog,
  KnownDevicesSheet,
} from '../../../screens/settings/KnownDevicesSheet';
import { ForgetDialog } from '../../../screens/history/ForgetDialog';
import { aboutModel } from '../../../flows/settings/settingsViews';
import { AboutSheet, CrashDialog, UpdateDialog } from '../../../screens/settings/SettingsDialogs';
import { UndoConfirmDialog } from '../../../screens/undo/UndoConfirmDialog';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';

export function ForgetOverlay({ id }: { readonly id: string }) {
  const { history } = useControllers();
  const nav = useNav();
  const record = history.records.find((r) => r.id === id);
  return (
    <ForgetDialog
      undoable={record !== undefined && historyEntry(record).undoable}
      on={{
        onKeep: nav.close,
        onForget: () => {
          history.dismiss(id);
          nav.close();
        },
      }}
    />
  );
}

export function UndoConfirmOverlay() {
  const { undo } = useControllers();
  const nav = useNav();
  if (undo.target === null) return null;
  return (
    <UndoConfirmDialog
      model={undo.target.preview}
      on={{
        onKeep: nav.close,
        onUndo: () => {
          undo.confirm();
          nav.go({ type: 'enterFlow', at: { flow: 'undo', step: 'result' } });
        },
      }}
    />
  );
}

export function UpdateOverlay() {
  const { update } = useControllers();
  const nav = useNav();
  return (
    <UpdateDialog
      model={update.model}
      on={{ onLater: nav.close, onInstall: update.install, onRestart: update.restart }}
    />
  );
}

export function ReportOverlay() {
  const { report } = useControllers();
  const nav = useNav();
  return (
    <CrashDialog
      model={report.model ?? { report: [], draft: null }}
      on={{
        onDontSend: () => {
          report.close();
          nav.close();
        },
        onSend: report.send,
        onDelete: () => {
          report.remove();
          nav.close();
        },
      }}
    />
  );
}

/** Versions and this computer's name as the engine reports them; licence is fixed. */
export function AboutOverlay() {
  const { settings } = useControllers();
  const nav = useNav();
  return <AboutSheet model={aboutModel(settings.about)} on={{ onClose: nav.close }} />;
}

/** Remembered devices: rename or forget each; Forget all asks first. */
export function KnownDevicesOverlay() {
  const { devices } = useControllers();
  const nav = useNav();
  return (
    <KnownDevicesSheet
      devices={devices.known.map(knownDeviceLine)}
      now={new Date()}
      on={{
        onRename: devices.rename,
        onForget: (id) => {
          devices.forget({ device: id });
        },
        onForgetAll: () => {
          nav.open({ kind: 'forget-devices', what: 'all' });
        },
        onDone: nav.close,
      }}
    />
  );
}

/** Forget every remembered device, or give this computer a new identity, once confirmed. */
export function ForgetDevicesOverlay({ what }: { readonly what: 'all' | 'identity' }) {
  const { devices } = useControllers();
  const nav = useNav();
  return (
    <ForgetDevicesDialog
      what={what}
      on={{
        onCancel: nav.close,
        onConfirm: () => {
          if (what === 'all') devices.forget('all');
          else devices.resetIdentity();
          nav.close();
        },
      }}
    />
  );
}
