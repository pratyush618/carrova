import { useCallback, useEffect, useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { NotifyAppsView } from '../../bindings/NotifyAppsView';
import { usePort } from '../../engine/port';
import { Group } from '../../screens/settings/Group';
import { AppGlyph, Banner, Button, Row, Search, Toggle } from '../../ui';
import { PhonePage } from '../PhonePage';
import { appsMatching } from './model';
import { usePermSteps } from './usePermSteps';
import '../../screens/settings/settings.css';

const ACCESS = ['notification_access'] as const;

/**
 * Which apps' notifications reach the linked computers (one list for all
 * of them), and what of each goes. Without notification access the list
 * shows dimmed under a way to grant it.
 */
export function NotifyApps({
  name,
  onBack,
  onAllow,
}: {
  /** The computer whose page this was opened from. */
  readonly name: string;
  readonly onBack: () => void;
  /** Grant notification access. */
  readonly onAllow: () => void;
}) {
  const { t } = useTranslation();
  const port = usePort();
  const [view, setView] = useState<NotifyAppsView | null>(null);
  const [query, setQuery] = useState('');
  const [failed, setFailed] = useState(false);
  const steps = usePermSteps(ACCESS);
  const noAccess = steps !== null && steps[0] !== 'ok';
  const load = useCallback(() => {
    port.notifyApps().then(setView, () => undefined);
  }, [port]);
  // Again each time the access step is read (back from settings): with
  // access, the apps showing notifications join the list.
  useEffect(load, [load, steps]);
  if (view === null)
    return (
      <PhonePage title={t('mobile.notify.title')} onBack={onBack}>
        {null}
      </PhonePage>
    );

  // A change shows at once; if it cannot be saved, the saved list comes back.
  const save = (next: NotifyAppsView, saving: Promise<void>) => {
    setView(next);
    setFailed(false);
    saving.catch(() => {
      setFailed(true);
      load();
    });
  };
  const setApp = (id: string, on: boolean) => {
    save(
      { ...view, apps: view.apps.map((a) => (a.id === id ? { ...a, on } : a)) },
      port.setNotifyApp(id, on),
    );
  };
  const setFilters = (hideCodes: boolean, showText: boolean) => {
    save(
      { ...view, hide_codes: hideCodes, show_text: showText },
      port.setNotifyFilters(hideCodes, showText),
    );
  };
  const shown = appsMatching(view.apps, query);
  return (
    <PhonePage title={t('mobile.notify.title')} onBack={onBack}>
      {noAccess ? (
        <>
          <Banner
            tone="warning"
            icon="lock"
            title={t('mobile.notify.noAccess')}
            body={t('mobile.notify.noAccessBody')}
          />
          <Button kind="primary" onClick={onAllow}>
            {t('mobile.notify.open')}
          </Button>
        </>
      ) : (
        <Search
          label={t('mobile.notify.search')}
          placeholder={t('mobile.notify.search')}
          value={query}
          onChange={setQuery}
        />
      )}
      {failed && <Banner tone="warning" title={t('ecosystem.device.failed')} />}
      <Group
        title={t('mobile.notify.showOn', { name })}
        {...(shown.length === 0 && {
          sub: t(view.apps.length === 0 ? 'mobile.notify.none' : 'mobile.notify.noMatch'),
        })}
      >
        {shown.map((app) => (
          <Row
            key={app.id}
            off={noAccess}
            lead={<AppGlyph glyph={app.name.slice(0, 2)} size="sm" />}
            title={app.name}
            {...(app.private && { sub: t('mobile.notify.private') })}
            trail={
              <Toggle
                on={app.on}
                label={app.name}
                onChange={(on) => {
                  setApp(app.id, on);
                }}
              />
            }
          />
        ))}
      </Group>
      <Group title={t('mobile.notify.content')}>
        <Row
          off={noAccess}
          title={t('mobile.notify.hideCodes')}
          sub={t('mobile.notify.hideCodesSub')}
          trail={
            <Toggle
              on={view.hide_codes}
              label={t('mobile.notify.hideCodes')}
              onChange={(on) => {
                setFilters(on, view.show_text);
              }}
            />
          }
        />
        <Row
          off={noAccess}
          title={t('mobile.notify.showText')}
          sub={t('mobile.notify.showTextSub')}
          trail={
            <Toggle
              on={view.show_text}
              label={t('mobile.notify.showText')}
              onChange={(on) => {
                setFilters(view.hide_codes, on);
              }}
            />
          }
        />
      </Group>
    </PhonePage>
  );
}
