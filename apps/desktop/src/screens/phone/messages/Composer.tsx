import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { SimChoice } from '../../../bindings/SimChoice';
import { presetSim } from '../../../flows/phone/usePhoneSims';
import { IconButton, Input } from '../../../ui';
import { cx } from '../../../ui/cx';
import { SimPicker } from '../SimPicker';

/**
 * The SIM to send from (dual-SIM phones only), the text field and Send.
 * While the phone is away it is off and says when sending works again:
 * nothing is queued.
 */
export function Composer({
  phone,
  away,
  disabled = false,
  sims,
  preferredSim = null,
  onSend,
}: {
  readonly phone: string;
  readonly away: boolean;
  /** Not ready to send yet (a new message with no recipient). */
  readonly disabled?: boolean;
  /** The SIMs to choose from; none with one SIM; `null` until the phone answers (no sending yet). */
  readonly sims: readonly SimChoice[] | null;
  /** The SIM to preselect while offered (the conversation's last), else the phone's default. */
  readonly preferredSim?: number | null;
  readonly onSend: (text: string, sim: number | null) => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  const [picked, setPicked] = useState<number | null>(null);
  const sim = presetSim(sims ?? [], picked ?? preferredSim);
  // Not before the phone says its SIMs: a reply would leave from its default, not the thread's.
  const off = away || disabled || sims === null;
  const send = () => {
    if (off || text.trim() === '') return;
    onSend(text, sim);
    setText('');
  };
  return (
    <form
      className={cx('s-composer', away && 's-composer--off')}
      onSubmit={(e) => {
        e.preventDefault();
        send();
      }}
    >
      <SimPicker
        label={t('ecosystem.messages.sendFrom')}
        sims={sims ?? []}
        value={sim}
        onChange={setPicked}
      />
      <Input
        aria-label={t('ecosystem.messages.field')}
        placeholder={
          away ? t('ecosystem.messages.awayHint', { name: phone }) : t('ecosystem.messages.field')
        }
        disabled={away}
        value={text}
        onChange={(e) => {
          setText(e.target.value);
        }}
      />
      <IconButton
        icon="send"
        label={t('ecosystem.messages.send')}
        disabled={off || text.trim() === ''}
        onClick={send}
      />
    </form>
  );
}
