import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  ActionBar,
  Banner,
  Button,
  FlowLayout,
  GroupHead,
  Head,
  List,
  Row,
  Seal,
  Tick,
} from '../../ui';
import { MoveRail, RouteChip, useComputerName } from '../shared/MoveRail';
import { LeftForYou } from './LeftForYou';
import type { ArrivedIntents, ArrivedModel } from './model';
import './receive.css';

export interface RecvDoneScreenProps {
  readonly model: ArrivedModel;
  readonly on: ArrivedIntents;
}

/** Apps listed by name before the rest fold into "and N more". */
const SHOWN = 5;

function DoneRail({ model }: { readonly model: ArrivedModel }) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <MoveRail
      route={model.route}
      here="to"
      notes={{ from: t('transfer.move.sent'), to: t('transfer.move.arrived') }}
      facts={[
        {
          label: t('receive.done.rail.apps'),
          value: t('receive.done.rail.inPlace', { count: model.moved.length }),
        },
        {
          label: t('receive.done.rail.files'),
          value: t('receive.done.rail.filesValue', {
            added: format.number(model.files - model.replaced),
            replaced: format.number(model.replaced),
          }),
        },
        { label: t('transfer.rail.size'), value: format.bytes(model.bytes) },
      ]}
      promise={t('receive.done.rail.promise')}
    />
  );
}

/** The receipt: apps that arrived, ticked, and the totals underneath. */
function WhatMoved({ model }: { readonly model: ArrivedModel }) {
  const { t } = useTranslation();
  const more = model.moved.length - SHOWN;
  return (
    <section aria-label={t('receive.done.moved')}>
      <GroupHead title={t('receive.done.moved')} />
      <List label={t('receive.done.moved')}>
        {model.moved.slice(0, SHOWN).map((app) => (
          <Row key={app} lead={<Tick state="done" />} title={app} />
        ))}
        {more > 0 && <Row title={t('receive.done.more', { count: more })} />}
      </List>
      <p className="s-receipt__foot">
        <span>{t('receive.done.files', { count: model.files })}</span>
        <span>{t('receive.done.backedUp', { count: model.replaced })}</span>
        <span>{t('receive.done.problems', { count: model.failures.length })}</span>
      </p>
    </section>
  );
}

/** The seal moment, the receipt, then what's left to install. Undo sits at the start of the bar. */
export function RecvDoneScreen({ model, on }: RecvDoneScreenProps) {
  const { t } = useTranslation();
  const name = useComputerName();
  const first = model.failures[0];
  return (
    <FlowLayout
      rail={<DoneRail model={model} />}
      route={<RouteChip route={model.route} here="to" />}
      actions={
        <ActionBar
          start={
            on.onUndo && (
              <Button kind="quiet" icon="undo" onClick={on.onUndo}>
                {t('receive.done.undo')}
              </Button>
            )
          }
          note={t('receive.done.saved')}
          end={
            <Button kind="primary" onClick={on.onDone}>
              {t('action.done')}
            </Button>
          }
        />
      }
    >
      {first === undefined ? (
        <div className="s-arrived">
          <Seal ring={t('receive.done.ring')} label={t('receive.done.sealLabel')} />
          <Head
            hero
            title={t('receive.done.title')}
            lede={t('receive.done.lede', {
              count: model.moved.length,
              from: name(model.route.from, false),
            })}
          />
        </div>
      ) : (
        <>
          <Head
            title={t('receive.done.titlePartial')}
            lede={t('receive.done.ledePartial', { what: first.what })}
          />
          <Banner
            tone="danger"
            title={t('receive.done.failed', { count: model.failures.length })}
            body={t('receive.done.failedBody', { what: first.what, reason: first.reason })}
            action={
              on.onRetry && (
                <Button size="sm" onClick={on.onRetry}>
                  {t('action.tryAgain')}
                </Button>
              )
            }
          />
        </>
      )}
      <div className="s-arrived__cols">
        <WhatMoved model={model} />
        {model.toInstall.length > 0 && <LeftForYou apps={model.toInstall} />}
      </div>
    </FlowLayout>
  );
}
