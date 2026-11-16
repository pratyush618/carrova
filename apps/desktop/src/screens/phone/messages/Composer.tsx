import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { IconButton, Input } from '../../../ui';
import { cx } from '../../../ui/cx';

/**
 * The text field and Send. While the phone is away it is off and says
 * when sending works again: nothing is queued.
 */
export function Composer({
  phone,
  away,
  disabled = false,
  onSend,
}: {
  readonly phone: string;
  readonly away: boolean;
  /** Not ready to send yet (a new message with no recipient). */
  readonly disabled?: boolean;
  readonly onSend: (text: string) => void;
}) {
  const { t } = useTranslation();
  const [text, setText] = useState('');
  const off = away || disabled;
  const send = () => {
    if (off || text.trim() === '') return;
    onSend(text);
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
