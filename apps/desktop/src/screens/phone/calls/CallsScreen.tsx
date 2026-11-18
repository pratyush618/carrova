import type { TFunction } from 'i18next';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { CallLogView } from '../../../bindings/CallLogView';
import type { Contact } from '../../../bindings/Contact';
import type { SimChoice } from '../../../bindings/SimChoice';
import { dialable, filterLog } from '../../../flows/calls/callSearch';
import { presetSim } from '../../../flows/phone/usePhoneSims';
import {
  Banner,
  Button,
  Empty,
  Field,
  Head,
  HubLayout,
  IconButton,
  Input,
  LinkButton,
  Row,
  Status,
} from '../../../ui';
import { Group } from '../../settings/Group';
import { SimPicker } from '../SimPicker';
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
  /** The SIMs a call can go from; none with one SIM. */
  readonly sims: readonly SimChoice[];
  /** The phone's contacts matching what was typed. */
  readonly contacts: readonly Contact[];
}

export interface CallsIntents {
  /** Call `number` from the phone, from SIM `sim` (`null`: its default). */
  readonly onDial: (number: string, sim: number | null) => void;
  readonly onTurnOn: () => void;
  readonly onMore: () => void;
  /** The name or number field changed: find the phone's contacts matching it. */
  readonly onSearch: (text: string) => void;
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
  const [picked, setPicked] = useState<number | null>(null);
  const { phone, log } = model;
  const sim = presetSim(model.sims, picked);
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
  const dial = dialable(number);
  const shown = log === null ? null : filterLog(log, number);
  const call = () => {
    if (dial !== '') on.onDial(dial, sim);
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
                on.onSearch(event.target.value);
              }}
            />
          )}
        </Field>
        <SimPicker
          label={t('calls.log.callFrom')}
          sims={model.sims}
          value={sim}
          onChange={setPicked}
        />
        <Button kind="primary" icon="call" disabled={dial === ''} onClick={call}>
          {t('calls.log.call')}
        </Button>
      </form>
      {model.contacts.length > 0 && (
        <Group title={t('calls.log.contacts')}>
          {model.contacts.map((c) => (
            <Row
              key={`${c.name}-${c.number}`}
              title={c.name}
              sub={c.number}
              trail={
                <IconButton
                  icon="call"
                  label={t('calls.log.callContact', { who: c.name })}
                  onClick={() => {
                    on.onDial(dialable(c.number), sim);
                  }}
                />
              }
            />
          ))}
        </Group>
      )}
      {model.failed && <Banner tone="warning" title={t('calls.log.failed', { phone })} />}
      {log === null && !model.failed && <Status>{t('calls.log.loading', { phone })}</Status>}
      {log !== null && log.groups.length === 0 && (
        <Empty
          art="call"
          title={t('calls.log.emptyTitle')}
          body={t('calls.log.emptyBody', { phone })}
        />
      )}
      {shown?.groups.map((group) => (
        <Group
          key={`${String(group.days_ago)}-${String(group.rows[0]?.id)}`}
          title={dayLabel(t, group.days_ago, group.rows[0]?.when_ms ?? 0)}
        >
          {group.rows.map((row) => (
            <CallLogRow
              key={row.id}
              row={row}
              onCallBack={() => {
                on.onDial(row.caller.number, sim);
              }}
            />
          ))}
        </Group>
      ))}
      {log?.next != null && <LinkButton onClick={on.onMore}>{t('calls.log.more')}</LinkButton>}
    </HubLayout>
  );
}
