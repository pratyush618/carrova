import { useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import { catalogModel, type CatalogUi } from '../../../flows/catalog/catalogViews';
import { historyEntry, homeModel } from '../../../flows/history/historyViews';
import { AppsScreen } from '../../../screens/apps/AppsScreen';
import { HistoryScreen } from '../../../screens/history/HistoryScreen';
import { SettingsScreen } from '../../../screens/settings/SettingsScreen';
import { HomeScreen } from '../../../screens/start/HomeScreen';
import { WelcomeScreen } from '../../../screens/start/WelcomeScreen';
import { HubLayout } from '../../../ui';
import { useControllers } from '../../controllers/Controllers';
import { useNav } from '../nav';
import { useStartFlows } from './useStartFlows';

/** Home: the two verbs, and the last move with its Undo. */
export function HomeRoute() {
  const { t } = useTranslation();
  const { history } = useControllers();
  const nav = useNav();
  const start = useStartFlows();
  const { reload, records } = history;
  useEffect(reload, [reload]);
  const model = homeModel(records, new Date(), t('history.anotherComputer'));
  const last = records.find((r) => r.state === 'done' && r.checklist !== null);
  return (
    <HomeScreen
      model={model}
      on={{
        onSend: start.send,
        onReceive: start.receive,
        onUseFile: () => {
          nav.open({ kind: 'file-choose' });
        },
        onUndoLast: () => {
          if (last) start.askUndo(last);
        },
      }}
    />
  );
}

export function HistoryRoute() {
  const { history } = useControllers();
  const nav = useNav();
  const start = useStartFlows();
  const { reload, records } = history;
  useEffect(reload, [reload]);
  return (
    <HistoryScreen
      model={{ now: new Date(), entries: records.map(historyEntry) }}
      on={{
        onOpen: (id) => {
          nav.open({ kind: 'session', id });
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

/** First run: the promises and the error-report question (off unless the user turns it on). */
export function WelcomeRoute() {
  const { settings } = useControllers();
  const nav = useNav();
  const [reports, setReports] = useState(false);
  return (
    <WelcomeScreen
      model={{ errorReports: reports }}
      on={{
        onErrorReports: setReports,
        onStart: () => {
          settings.setConsent(reports);
          nav.go({ type: 'finishWelcome' });
        },
      }}
    />
  );
}
