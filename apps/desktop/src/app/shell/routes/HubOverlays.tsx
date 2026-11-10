import { recipeModel } from '../../../flows/catalog/catalogViews';
import { historyEntry, sessionModel } from '../../../flows/history/historyViews';
import { RecipeSheet } from '../../../screens/apps/RecipeSheet';
import { ForgetDialog } from '../../../screens/history/ForgetDialog';
import { SessionSheet } from '../../../screens/history/SessionSheet';
import { aboutModel } from '../../../flows/settings/settingsViews';
import { AboutSheet, CrashDialog, UpdateDialog } from '../../../screens/settings/SettingsDialogs';
import { UndoConfirmDialog } from '../../../screens/undo/UndoConfirmDialog';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';
import { useStartFlows } from './useStartFlows';

export function SessionOverlay({ id }: { readonly id: string }) {
  const { history } = useControllers();
  const nav = useNav();
  const start = useStartFlows();
  const record = history.records.find((r) => r.id === id);
  if (record === undefined) return null;
  return (
    <SessionSheet
      model={sessionModel(record)}
      on={{
        onRemoveUndoPoint: () => {
          nav.open({ kind: 'forget', id });
        },
        onUndo: () => {
          start.askUndo(record);
        },
        onResume: () => {
          start.resume(record);
        },
        onClose: nav.close,
      }}
    />
  );
}

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
          // Closes this dialog and the move's sheet under it.
          nav.close();
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

export function RecipeOverlay({ id }: { readonly id: string }) {
  const { catalog } = useControllers();
  const nav = useNav();
  const model = recipeModel(catalog.entries ?? [], id);
  return model === null ? null : <RecipeSheet model={model} on={{ onClose: nav.close }} />;
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
