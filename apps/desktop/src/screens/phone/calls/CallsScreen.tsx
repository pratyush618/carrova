import type { TFunction } from 'i18next';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CallLogView } from '../../../bindings/CallLogView';
import { Banner, Button, Empty, Field, Head, HubLayout, Input, LinkButton } from '../../../ui';
import { Group } from '../../settings/Group';
import { CallLogRow } from './CallLogRow';
import './calls.css';

export interface CallsModel {
  readonly phone: string;
  /** Calls are on for this phone (off: nothing rings here, no log). */
  readonly on: boolean;
  /** `null` until the phone answers. */
  readonly log: CallLogView | null;
  /** The log couldn't be read. */
  readonly failed: boolean;
}

export interface CallsIntents {
  /** Call `number` from the phone. */
  readonly onDial: (number: string) => void;
  readonly onTurnOn: () => void;
  readonly onMore: () => void;
}

/** "Today", "Yesterday" or the date of a day's first call. */
function dayLabel(t: TFunction, daysAgo: number, firstMs: number): string {
  if (daysAgo === 0) return t('calls.log.today');
  if (daysAgo === 1) return t('calls.log.yesterday');
  return new Date(firstMs).toLocaleDateString([], { day: 'numeric', month: 'long' });
}

/** A linked phone's calls: call a number from it, and its call log by day. */
export function CallsScreen({
  model,
  on,
}: {
  readonly model: CallsModel;
  readonly on: CallsIntents;
}) {
  const { t } = useTranslation();
  const [number, setNumber] = useState('');
  const { phone, log } = model;
  if (!model.on)
    return (
      <HubLayout variant="center">
        <Empty
          art="call"
          title={t('calls.log.offTitle', { phone })}
          body={t('calls.log.offBody')}
          action={
            <Button kind="primary" icon="call" onClick={on.onTurnOn}>
              {t('calls.log.turnOn')}
            </Button>
          }
        />
      </HubLayout>
    );
  const dial = number.trim();
  const call = () => {
    if (dial !== '') on.onDial(dial);
  };
  return (
    <HubLayout>
      <Head title={t('calls.log.title')} lede={phone} />
      <form
        className="s-calls__dial"
        onSubmit={(event) => {
          event.preventDefault();
          call();
        }}
      >
        <Field label={t('calls.log.dial')}>
          {(control) => (
            <Input
              {...control}
              type="tel"
              autoComplete="off"
              value={number}
              onChange={(event) => {
                setNumber(event.target.value);
              }}
            />
          )}
        </Field>
        <Button kind="primary" icon="call" disabled={dial === ''} onClick={call}>
          {t('calls.log.call')}
        </Button>
      </form>
      {model.failed && <Banner tone="warning" title={t('calls.log.failed', { phone })} />}
      {log !== null && log.groups.length === 0 && (
        <Empty
          art="call"
          title={t('calls.log.emptyTitle')}
          body={t('calls.log.emptyBody', { phone })}
        />
      )}
      {log?.groups.map((group) => (
        <Group
          key={`${String(group.days_ago)}-${String(group.rows[0]?.id)}`}
          title={dayLabel(t, group.days_ago, group.rows[0]?.when_ms ?? 0)}
        >
          {group.rows.map((row) => (
            <CallLogRow
              key={row.id}
              row={row}
              onCallBack={() => {
                on.onDial(row.caller.number);
              }}
            />
          ))}
        </Group>
      ))}
      {log?.next != null && <LinkButton onClick={on.onMore}>{t('calls.log.more')}</LinkButton>}
    </HubLayout>
  );
}
