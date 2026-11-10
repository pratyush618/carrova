import { useEffect, useState } from 'react';
import { catalogModel, type CatalogUi } from '../../../flows/catalog/catalogViews';
import { historyEntry, homeModel, sessionModel } from '../../../flows/history/historyViews';
import { useUndoPreview } from '../../../flows/undo/useUndoPreview';
import { current, type Remote } from '../../../flows/remote';
import { AppsScreen } from '../../../screens/apps/AppsScreen';
import { HistoryScreen } from '../../../screens/history/HistoryScreen';
import { SettingsScreen } from '../../../screens/settings/SettingsScreen';
import { HomeScreen } from '../../../screens/start/HomeScreen';
import { HubLayout } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';
import { useStartFlows } from './useStartFlows';

/** A list the page can't render until it has answered once (a failure counts: render empty). */
const answered = (remote: Remote<unknown>): boolean =>
  remote.status === 'ready' || remote.status === 'failed' || current(remote) !== null;

/** Home: the two verbs, recent moves with Undo, the one-time error-report question. */
export function HomeRoute() {
  const { history, settings } = useControllers();
  const nav = useNav();
  const start = useStartFlows();
  const { reload, records, sessions } = history;
  useEffect(reload, [reload]);
  // Rendering before sessions load would flash the first-run Home at returning users.
  if (!answered(sessions)) return <HubLayout variant="center">{null}</HubLayout>;
  return (
    <HomeScreen
      model={homeModel(records, new Date(), settings.consent)}
      on={{
        onSend: start.send,
        onReceive: start.receive,
        onUseFile: () => {
          nav.open({ kind: 'file-choose' });
        },
        onUndo: (id) => {
          const record = records.find((r) => r.id === id);
          if (record) start.askUndo(record);
        },
        onHistory: () => {
          nav.go({ type: 'openHub', page: 'history' });
        },
        onErrorReports: settings.setConsent,
      }}
    />
  );
}

/** History: every move, the selected one (newest by default) open beside the list. */
export function HistoryRoute() {
  const { history, undo } = useControllers();
  const nav = useNav();
  const start = useStartFlows();
  const [picked, setPicked] = useState<string | null>(null);
  const { reload, records, sessions } = history;
  useEffect(reload, [reload]);
  const record = records.find((r) => r.id === picked) ?? records[0];
  const preview = useUndoPreview(
    undo.peek,
    record !== undefined && historyEntry(record).undoable ? record.id : null,
  );
  if (!answered(sessions)) return <HubLayout>{null}</HubLayout>;
  return (
    <HistoryScreen
      model={{
        now: new Date(),
        entries: records.map(historyEntry),
        selected: record === undefined ? null : sessionModel(record, preview),
      }}
      on={{
        onSelect: setPicked,
        onUndo: () => {
          if (record) start.askUndo(record);
        },
        onResume: () => {
          if (record) start.resume(record);
        },
        onForget: () => {
          if (record) nav.open({ kind: 'forget', id: record.id });
        },
        onReceive: start.receive,
      }}
    />
  );
}

export function AppsRoute() {
  const { catalog } = useControllers();
  const nav = useNav();
  const [ui, setUi] = useState<CatalogUi>({ filter: 'all', query: '' });
  const model = catalogModel(catalog.entries ?? [], catalog.status, ui, new Date());
  return (
    <AppsScreen
      model={model}
      on={{
        onFilter: (filter) => {
          setUi((u) => ({ ...u, filter }));
        },
        onQuery: (query) => {
          setUi((u) => ({ ...u, query }));
        },
        onOpen: (id) => {
          nav.open({ kind: 'recipe', id });
        },
      }}
    />
  );
}

export function SettingsRoute() {
  const { settings, update, report } = useControllers();
  const nav = useNav();
  if (settings.model === null) return <HubLayout>{null}</HubLayout>;
  return (
    <SettingsScreen
      model={settings.model}
      on={{
        onAppearance: (value) => {
          settings.save({ theme: value === 'system' ? null : value });
        },
        onMotion: (value) => {
          settings.save({ reduce_motion: value === 'reduce' ? true : null });
        },
        onAutoCheckRecipes: (on) => {
          settings.save({ auto_check_recipes: on });
        },
        onAutoCheckApp: (on) => {
          settings.save({ auto_check_app: on });
        },
        onCheckRecipes: settings.checkRecipes,
        onErrorReports: settings.setConsent,
        onOpenReport: (id) => {
          report.open(id);
          nav.open({ kind: 'report', id });
        },
        onClearReports: settings.clearReports,
        onUpdates: () => {
          update.runCheck();
          nav.open({ kind: 'update' });
        },
        onAbout: () => {
          nav.open({ kind: 'about' });
        },
        onMoveDefaults: (patch) => {
          const current = settings.settings?.move_defaults;
          if (current !== undefined) settings.save({ move_defaults: { ...current, ...patch } });
        },
      }}
    />
  );
}
