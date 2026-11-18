import { useTranslation } from 'react-i18next';
import type { CallRow } from '../../../bindings/CallRow';
import { AppGlyph, IconButton, Row } from '../../../ui';

/** One call of the log: who, how it went and how long, when, and call back. */
export function CallLogRow({
  row,
  onCallBack,
}: {
  readonly row: CallRow;
  readonly onCallBack: () => void;
}) {
  const { t } = useTranslation();
  const who = row.caller.name ?? row.caller.number;
  const length = t('calls.log.minutes', { count: Math.max(1, Math.ceil(row.duration_s / 60)) });
  return (
    <Row
      lead={
        <AppGlyph
          glyph={{ icon: 'call' }}
          size="sm"
          tone={row.kind === 'missed' ? 'warning' : 'neutral'}
        />
      }
      title={who}
      sub={t(`calls.log.kinds.${row.kind}`, { length })}
      trail={
        <>
          <span className="p-mono">
            {new Date(row.when_ms).toLocaleTimeString([], { hour: '2-digit', minute: '2-digit' })}
          </span>
          <IconButton icon="call" label={t('calls.log.callBack', { who })} onClick={onCallBack} />
        </>
      }
    />
  );
}
