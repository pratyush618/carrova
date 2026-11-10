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
  type RouteEnd,
} from '../../ui';
import { FoundSelection } from './FoundSelection';
import { FOUND_PRESETS, type FoundIntents, type FoundModel, type FoundPurpose } from './model';
import { RouteChip, SendRail } from './SendRail';
import './send.css';

export interface SendFoundScreenProps {
  readonly model: FoundModel;
  readonly purpose: FoundPurpose;
  /** This computer, once the engine has said (route chip and rail). */
  readonly here?: RouteEnd | null;
  readonly on: FoundIntents;
}

/** "Here's what we found": the one selection screen for sending and for saving a file. */
export function SendFoundScreen({ model, purpose, here = null, on }: SendFoundScreenProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const back = (
    <Button kind="quiet" icon="back" onClick={on.onBack}>
      {t('action.back')}
    </Button>
  );

  switch (model.status) {
    case 'scanning':
      return (
        <FlowLayout
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
        <FlowLayout actions={<ActionBar start={back} />}>
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
          {...(purpose === 'send' && { route: <RouteChip here={here} /> })}
          rail={<SendRail move={{ here, facts }} purpose={purpose} onChange={on.onCustomize} />}
          actions={
            <ActionBar
              start={
                <>
                  <Button onClick={on.onCustomize}>{t('send.found.customize')}</Button>
                  {on.onSaveInstead && (
                    <Button kind="quiet" icon="archive" onClick={on.onSaveInstead}>
                      {t('send.found.saveInstead')}
                    </Button>
                  )}
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
                  {purpose === 'save' ? t('action.continue') : t('send.found.toPairing')}
                </Button>
              }
            />
          }
        >
          <Head
            title={t('send.found.title')}
            lede={t('send.found.lede', {
              apps: t('count.apps', { count: facts.totalApps }),
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
            <Note center>{t('send.found.noMatch', { filter: model.filter.trim() })}</Note>
          ) : (
            <FoundSelection groups={model.groups} secrets={model.secrets} on={on} />
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
  }
}
