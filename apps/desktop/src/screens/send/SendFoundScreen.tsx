import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  ActionBar,
  Banner,
  Button,
  Empty,
  FlowLayout,
  Head,
  LinkButton,
  Note,
  Progress,
  Search,
  Segmented,
  Status,
  Toggle,
  type RouteEnd,
} from '../../ui';
import { FoundSelection } from './FoundSelection';
import { ManualPicks } from './ManualPicks';
import {
  FOUND_FILTERS,
  FOUND_PRESETS,
  type FoundIntents,
  type FoundModel,
  type FoundPurpose,
  type ModeSwitchModel,
} from './model';
import { RouteChip, SendRail } from './SendRail';
import './send.css';

export interface SendFoundScreenProps {
  readonly model: FoundModel;
  readonly purpose: FoundPurpose;
  /** This computer, once the engine has said (route chip and rail). */
  readonly here?: RouteEnd | null;
  /** The Auto/Manual switch; absent where the screen offers none. */
  readonly mode?: ModeSwitchModel;
  readonly on: FoundIntents;
}

/** "Manual" on/off, with what the other mode holds ("Manual · 2"). */
function ModeSwitch({
  mode,
  onMode,
}: {
  readonly mode: ModeSwitchModel;
  readonly onMode: (manual: boolean) => void;
}) {
  const { t } = useTranslation();
  const label =
    mode.manual && mode.apps > 0
      ? t('send.manual.switchApps', { apps: t('count.apps', { count: mode.apps }) })
      : !mode.manual && mode.picks > 0
        ? t('send.manual.switchPicks', { count: mode.picks })
        : t('send.manual.switch');
  return (
    <span className="s-mode">
      <span aria-hidden="true">{label}</span>
      <Toggle on={mode.manual} label={label} disabled={mode.disabled} onChange={onMode} />
    </span>
  );
}

/** "Here's what we found": the one selection screen for sending and for saving a file. */
export function SendFoundScreen({ model, purpose, here = null, mode, on }: SendFoundScreenProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const modeSwitch = mode && on.onMode ? <ModeSwitch mode={mode} onMode={on.onMode} /> : null;
  const route = (
    <>
      {modeSwitch}
      {purpose === 'send' && <RouteChip here={here} />}
    </>
  );
  const saveInstead = on.onSaveInstead && (
    <Button kind="quiet" icon="archive" onClick={on.onSaveInstead}>
      {t('send.found.saveInstead')}
    </Button>
  );
  const continueLabel = purpose === 'save' ? t('action.continue') : t('send.found.toPairing');
  const back = (
    <Button kind="quiet" icon="back" onClick={on.onBack}>
      {t('action.back')}
    </Button>
  );

  switch (model.status) {
    case 'scanning':
      return (
        <FlowLayout
          route={modeSwitch}
          actions={
            <ActionBar
              start={
                <Button kind="quiet" onClick={on.onBack}>
                  {t('action.cancel')}
                </Button>
              }
            />
          }
        >
          <Head title={t('send.found.scanningTitle')} lede={t('send.found.scanningLede')} />
          <Status>
            {model.foundSoFar.length > 0
              ? t('send.found.foundSoFar', { apps: model.foundSoFar.join(', ') })
              : t('send.found.looking')}
          </Status>
          {model.progress !== null && (
            <Progress value={model.progress} label={t('send.found.scanProgress')} />
          )}
        </FlowLayout>
      );
    case 'nothing':
      return (
        <FlowLayout route={modeSwitch} actions={<ActionBar start={back} />}>
          <Empty
            art="search"
            title={t('send.found.nothingTitle')}
            body={t('send.found.nothingBody', { count: model.knownApps })}
            action={
              <Button kind="primary" onClick={on.onPickFolders}>
                {t('send.found.pickFolders')}
              </Button>
            }
          />
          <Note center>
            <LinkButton onClick={on.onSeeKnownApps}>{t('send.found.seeKnown')}</LinkButton>
          </Note>
        </FlowLayout>
      );
    case 'found': {
      const { facts } = model;
      const empty = model.groups.length === 0 && model.secrets.length === 0;
      const note = {
        apps: t('count.apps', { count: facts.apps }),
        size: format.bytes(facts.bytes),
      };
      return (
        <FlowLayout
          route={route}
          rail={<SendRail move={{ here, facts }} purpose={purpose} onChange={on.onCustomize} />}
          actions={
            <ActionBar
              start={
                <>
                  <Button onClick={on.onCustomize}>{t('send.found.customize')}</Button>
                  {saveInstead}
                </>
              }
              note={
                facts.keys === 0
                  ? t('send.found.noteNoKeys', note)
                  : t('send.found.noteWithKeys', { ...note, count: facts.keys })
              }
              end={
                <Button
                  kind="primary"
                  disabled={model.issues.length > 0 || model.plugins.length > 0}
                  onClick={on.onContinue}
                >
                  {continueLabel}
                </Button>
              }
            />
          }
        >
          <Head
            title={t('send.found.title')}
            lede={t('send.found.lede', {
              apps: t('count.apps', { count: facts.totalApps }),
              ticked: facts.apps,
              device: here?.name ?? t('send.found.thisComputer'),
            })}
          />
          {model.issues.map((issue) => (
            <Banner key={issue} tone="danger" title={t('send.found.issue')} body={issue} />
          ))}
          {model.plugins.map(({ plugin, items }) => (
            <Banner
              key={plugin}
              tone="warning"
              icon="plug"
              title={t('send.found.plugin', { plugin, what: items.join(', ') })}
              action={
                <Button
                  size="sm"
                  aria-label={t('send.found.pluginReviewFor', { plugin })}
                  onClick={() => {
                    on.onReviewPlugin(plugin);
                  }}
                >
                  {t('send.found.pluginReview')}
                </Button>
              }
            />
          ))}
          <div className="s-found-tools">
            <Search
              label={t('send.found.filter')}
              placeholder={t('send.found.filter')}
              value={model.filter}
              onChange={on.onFilter}
            />
            <Segmented
              label={t('send.found.show')}
              options={FOUND_FILTERS.map((value) => ({
                value,
                label: t(`send.found.view.${value}`),
              }))}
              value={model.foundFilter}
              onChange={on.onFoundFilter}
            />
            <Segmented
              label={t('send.found.presets')}
              options={FOUND_PRESETS.map((value) => ({
                value,
                label: t(`send.found.preset.${value}`),
              }))}
              value={model.preset}
              onChange={on.onPreset}
            />
          </div>
          {empty ? (
            <Note center>
              {model.filter.trim() === ''
                ? t('send.found.noneInView')
                : t('send.found.noMatch', { filter: model.filter.trim() })}
            </Note>
          ) : (
            <FoundSelection
              groups={model.groups}
              secrets={model.secrets}
              narrowing={`${model.foundFilter}:${model.filter}`}
              on={on}
            />
          )}
          {model.unrecognised > 0 && (
            <Note>
              {t('send.found.notListed')}{' '}
              <LinkButton onClick={on.onPickFolders}>
                {t('send.found.unrecognised', { count: model.unrecognised })}
              </LinkButton>
            </Note>
          )}
        </FlowLayout>
      );
    }
    case 'manual': {
      const { facts } = model;
      const note = {
        items: t('send.manual.items', { count: facts.picks ?? 0 }),
        size: format.bytes(facts.bytes),
      };
      return (
        <FlowLayout
          route={route}
          rail={<SendRail move={{ here, facts }} purpose={purpose} />}
          actions={
            <ActionBar
              start={saveInstead}
              note={
                !facts.secrets
                  ? t('send.manual.noteNoKeys', note)
                  : t('send.manual.noteWithKeys', { ...note, count: facts.secrets })
              }
              end={
                <Button
                  kind="primary"
                  disabled={model.issues.length > 0 || facts.picks === 0}
                  onClick={on.onContinue}
                >
                  {continueLabel}
                </Button>
              }
            />
          }
        >
          <Head title={t('send.manual.title')} lede={t('send.manual.lede')} />
          {model.issues.map((issue) => (
            <Banner key={issue} tone="danger" title={t('send.found.issue')} body={issue} />
          ))}
          <ManualPicks model={model} on={on} />
        </FlowLayout>
      );
    }
  }
}
