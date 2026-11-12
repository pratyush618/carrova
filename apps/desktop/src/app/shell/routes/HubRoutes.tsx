import { useEffect, useState } from 'react';
import { catalogModel, type CatalogUi } from '../../../flows/catalog/catalogViews';
import { historyEntry, homeModel, sessionModel } from '../../../flows/history/historyViews';
import { nearbyEntries, visibilityModel, wakeFailure } from '../../../flows/nearby/nearbyViews';
import { useNearby } from '../../../flows/nearby/useNearby';
import { pausedUntil } from '../../../flows/settings/settingsViews';
import { useUndoPreview } from '../../../flows/undo/useUndoPreview';
import { current, type Remote } from '../../../flows/remote';
import { AppsScreen } from '../../../screens/apps/AppsScreen';
import { HistoryScreen } from '../../../screens/history/HistoryScreen';
import type { NearbyView, WakeNote } from '../../../screens/nearby/model';
import { NearbyScreen } from '../../../screens/nearby/NearbyScreen';
import type { SettingsSection } from '../../../screens/settings/sections';
import { SettingsScreen } from '../../../screens/settings/SettingsScreen';
import { HomeScreen } from '../../../screens/start/HomeScreen';
import { HubLayout } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';
import { useStartFlows } from './useStartFlows';

/** A list the page can't render until it has answered once (a failure counts: render empty). */
const answered = (remote: Remote<unknown>): boolean =>
  remote.status === 'ready' || remote.status === 'failed' || current(remote) !== null;

/** Home: the two verbs, recent moves with Undo, the one-time questions (error reports, file manager). */
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
      model={homeModel(records, new Date(), settings.consent, {
        offer: settings.offerFileManager,
        failed: settings.fileManagerFailed,
      })}
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
        onFileManager: (yes) => {
          if (yes) settings.setFileManager(true);
          else settings.save({ file_manager_asked: true });
        },
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
        entries: records.map((r) => historyEntry(r)),
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

/** Nearby: looks again every few seconds while open; visibility and remembered devices are shared with Settings. */
export function NearbyRoute() {
  const { devices, settings } = useControllers();
  const nav = useNav();
  const start = useStartFlows();
  const [view, setView] = useState<NearbyView>('map');
  const [selected, setSelected] = useState<string | null>(null);
  const [wake, setWake] = useState<WakeNote | null>(null);
  const found = useNearby(devices.browse, true);
  return (
    <NearbyScreen
      model={{
        view,
        ...nearbyEntries(found, devices.known),
        selected,
        visibility: devices.visibility && visibilityModel(devices.visibility),
        me: settings.about?.device_name ?? null,
        wake,
        failed: devices.failed,
        now: new Date(),
      }}
      on={{
        onView: setView,
        onSelect: setSelected,
        onVisibility: devices.setVisibility,
        onSettings: () => {
          nav.go({ type: 'openHub', page: 'settings', section: 'nearby' });
        },
        onSendWithCode: start.send,
        onWake: ({ id, name }) => {
          if (id === null) return;
          void devices.wake(id).then(
            () => {
              setWake({ name, outcome: 'sent' });
            },
            (reason: unknown) => {
              setWake({ name, outcome: wakeFailure(reason) });
            },
          );
        },
        onForget: ({ id }) => {
          if (id !== null) void devices.forget({ device: id });
          setSelected(null);
        },
        onDismissWake: () => {
          setWake(null);
        },
      }}
    />
  );
}

export function AppsRoute() {
  const { catalog } = useControllers();
  const [ui, setUi] = useState<CatalogUi>({
    filter: 'all',
    query: '',
    selected: null,
    expanded: [],
  });
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
        onSelect: (selected) => {
          setUi((u) => ({ ...u, selected }));
        },
        onShowAll: (category) => {
          setUi((u) => ({ ...u, expanded: [...u.expanded, category] }));
        },
      }}
    />
  );
}

export function SettingsRoute({ section }: { readonly section: SettingsSection }) {
  const { settings, update, report, devices } = useControllers();
  const nav = useNav();
  const { model } = settings;
  if (model === null) return <HubLayout>{null}</HubLayout>;
  return (
    <SettingsScreen
      section={section}
      model={model}
      on={{
        onSection: (next) => {
          nav.go({ type: 'openHub', page: 'settings', section: next });
        },
        onBack: () => {
          nav.go({ type: 'back' });
        },
        onBackground: settings.setBackground,
        onNearby: (patch) => {
          settings.save({ nearby: { ...model.nearby, ...patch } });
        },
        onVisibility: devices.setVisibility,
        onKnownDevices: () => {
          nav.open({ kind: 'known-devices' });
        },
        onResetIdentity: () => {
          nav.open({ kind: 'forget-devices', what: 'identity' });
        },
        onAlerts: (patch) => {
          settings.save({ alerts: patch });
        },
        onPause: (choice) => {
          settings.save({ alerts: { paused_until: pausedUntil(choice, new Date()) } });
        },
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
        onInstallRecipes: settings.installRecipes,
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
        onStartInManual: (on) => {
          settings.save({ start_in_manual: on });
        },
        onFileManager: settings.setFileManager,
        onMoveDefaults: (patch) => {
          settings.save({ move_defaults: patch });
        },
      }}
    />
  );
}
