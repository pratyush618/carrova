import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { minutesLeft } from '../../i18n/format';
import {
  AppGlyph,
  Banner,
  Button,
  GroupHead,
  Head,
  Icon,
  List,
  LockLine,
  OsBadge,
  Progress,
  Row,
  Tick,
  type TickState,
} from '../../ui';
import { chipText } from './chipText';
import type { Computer, MoveRoute } from './model';
import { MoveRail, RouteChip, useComputerName } from './MoveRail';
import {
  interrupted,
  stoppedAt,
  type TransferApp,
  type TransferIntents,
  type TransferModel,
} from './transfer';
import './transfer.css';

export interface TransferBodyProps {
  readonly model: TransferModel;
  readonly on: Pick<TransferIntents, 'onOpenFiles'>;
}

/** The route, or as much of it as the model knows (the peer's name). */
function routeOf(model: TransferModel): MoveRoute {
  if (model.route !== null) return model.route;
  const peer: Computer = { name: model.peer, os: null };
  const here: Computer = { name: null, os: null };
  return model.side === 'send' ? { from: here, to: peer } : { from: peer, to: here };
}

function useTimeLeft(): (seconds: number) => string {
  const { t } = useTranslation();
  return (seconds) =>
    seconds < 60
      ? t('transfer.secondsLeft', { count: Math.max(1, Math.round(seconds)) })
      : t('summary.minutesLeft', { count: minutesLeft(seconds) });
}

/** The live card: percent, bytes, time left and rate, then the bar running from sender to receiver. */
function LiveCard({ model }: { readonly model: TransferModel }) {
  const { t } = useTranslation();
  const format = useFormat();
  const timeLeft = useTimeLeft();
  const name = useComputerName();
  const fraction = model.total === 0 ? 0 : model.done / model.total;
  const route = routeOf(model);
  const end = (computer: Computer, here: boolean) => (
    <span className="s-live__end">
      <span className="s-live__screen" aria-hidden="true">
        <Icon name="laptop" size="sm" />
      </span>
      <span>
        <b>{name(computer, here)}</b>
        {computer.os !== null && (
          <OsBadge os={computer.os} label={t(`settings.about.os.${computer.os}`)} size="sm" />
        )}
      </span>
    </span>
  );
  const status =
    model.phase === 'moving'
      ? [
          model.secondsLeft === null ? null : timeLeft(model.secondsLeft),
          model.bytesPerSecond === null
            ? null
            : t('transfer.rate', { size: format.bytes(model.bytesPerSecond) }),
          t('transfer.encrypted'),
        ]
          .filter((part) => part !== null)
          .join(' · ')
      : t(`transfer.status.${model.phase}`);
  return (
    <div className="s-live">
      <p className="s-live__top">
        <b className="s-live__pct">
          {t('transfer.percent', { value: format.number(Math.floor(fraction * 100)) })}
        </b>
        <span>
          {t('summary.ofTotal', {
            done: format.bytes(model.done),
            total: format.bytes(model.total),
          })}
        </span>
        <span className="s-live__status">{status}</span>
      </p>
      <div className="s-live__wire">
        {end(route.from, model.side === 'send')}
        <Progress
          value={fraction}
          label={t('ui.progress')}
          paused={model.phase !== 'moving' && model.phase !== 'applying'}
        />
        {end(route.to, model.side === 'receive')}
      </div>
    </div>
  );
}

const TICK: Record<TransferApp['state'], TickState> = {
  done: 'done',
  moving: 'now',
  waiting: 'queued',
  failed: 'queued',
};

/** One app moving, smallest first: tick, name, what's happening, its own bar while it moves. */
function AppRow({ app, model }: { readonly app: TransferApp; readonly model: TransferModel }) {
  const { t } = useTranslation();
  const format = useFormat();
  const stopped = interrupted(model.phase);
  return (
    <Row
      lead={
        <>
          <Tick state={stoppedAt(model) === app.id ? 'wait' : TICK[app.state]} />
          <AppGlyph glyph={app.glyph} size="sm" />
        </>
      }
      title={app.name}
      sub={chipText(app, model, t, format.bytes)}
      trail={
        <>
          {app.state === 'moving' && app.bytes > 0 && (
            <Progress
              size="sm"
              value={app.bytesDone / app.bytes}
              label={app.name}
              paused={stopped}
            />
          )}
          {format.bytes(app.bytes)}
        </>
      }
    />
  );
}

/** The apps list; once interrupted, finished apps fold into one line so what's left stands out. */
function AppList({ model }: { readonly model: TransferModel }) {
  const { t } = useTranslation();
  const done = model.apps.filter((a) => a.state === 'done');
  const fold = interrupted(model.phase) && done.length > 1;
  const rest = fold ? model.apps.filter((a) => a.state !== 'done') : model.apps;
  return (
    <section aria-label={t('transfer.apps')}>
      <GroupHead
        title={t('transfer.apps')}
        aside={
          <span className="s-live__count" aria-live="polite">
            {t('transfer.inPlace', { done: done.length, total: model.apps.length })}
          </span>
        }
      />
      <List label={t('transfer.apps')}>
        {fold && (
          <Row
            lead={
              <>
                <Tick state="done" />
                <AppGlyph glyph={String(done.length)} size="sm" />
              </>
            }
            title={t('transfer.folded', { count: done.length })}
            sub={done.map((a) => a.name).join(', ')}
          />
        )}
        {rest.map((app) => (
          <AppRow key={app.id} app={app} model={model} />
        ))}
      </List>
    </section>
  );
}

/** The progress body: live card, apps ticking off small-first, and the lock line. */
export function TransferBody({ model, on }: TransferBodyProps) {
  const { t } = useTranslation();
  const format = useFormat();
  const shortfall = format.bytes(model.shortfall);
  return (
    <>
      <Head
        title={t(`transfer.title.${model.phase}`)}
        lede={t(`transfer.lede.${model.phase}`, { size: shortfall })}
      />
      {model.phase === 'paused' && (
        <Banner
          tone="danger"
          title={t('transfer.spaceTitle', { size: shortfall })}
          body={t('transfer.spaceBody')}
          action={
            on.onOpenFiles && (
              <Button size="sm" onClick={on.onOpenFiles}>
                {t('transfer.openFiles')}
              </Button>
            )
          }
        />
      )}
      {(model.phase === 'held' || model.phase === 'reconnecting') && (
        <Banner
          tone="warning"
          title={t('transfer.safeTitle', { size: format.bytes(model.done) })}
          body={t('transfer.safeBody')}
        />
      )}
      <LiveCard model={model} />
      <AppList model={model} />
      <LockLine quiet>
        {model.side === 'send'
          ? t('transfer.lockSend', { peer: model.peer })
          : t('transfer.lockReceive')}
      </LockLine>
    </>
  );
}

/** The transfer's "This move" rail: the route goes live while bytes flow. */
export function TransferRail({ model }: { readonly model: TransferModel }) {
  const { t } = useTranslation();
  const format = useFormat();
  const send = model.side === 'send';
  const stopped = interrupted(model.phase);
  const working = stopped ? t('transfer.move.waiting') : t('transfer.move.receiving');
  return (
    <MoveRail
      route={routeOf(model)}
      here={send ? 'from' : 'to'}
      live={model.phase === 'moving'}
      notes={{ from: t('transfer.move.sending'), to: working }}
      facts={[
        { label: t('transfer.rail.apps'), value: format.number(model.apps.length) },
        { label: t('transfer.rail.size'), value: format.bytes(model.total) },
      ]}
      promise={send ? t('transfer.rail.promiseSend') : t('transfer.rail.promise')}
    />
  );
}

/** The header's route chip for a transfer, once both systems are known. */
export function TransferRoute({ model }: { readonly model: TransferModel }) {
  return <RouteChip route={routeOf(model)} here={model.side === 'send' ? 'from' : 'to'} />;
}
