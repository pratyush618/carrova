import { useTranslation } from 'react-i18next';
import { usePort } from '../engine/port';
import { useClipHistory } from '../flows/ecosystem/useClipHistory';
import { ClipList } from '../screens/clipboard/ClipHistorySheet';
import { Button } from '../ui';
import { PhonePage } from './PhonePage';

/** Activity: what the computers copied here, newest first; kept on this phone only. */
export function PhoneActivity() {
  const { t } = useTranslation();
  const { items, clear } = useClipHistory(usePort());
  return (
    <PhonePage
      title={t('mobile.activity.title')}
      actions={
        items !== null &&
        items.length > 0 && (
          <Button kind="quiet" size="sm" onClick={clear}>
            {t('ecosystem.history.clear')}
          </Button>
        )
      }
    >
      <p className="m-note">{t('mobile.activity.sub')}</p>
      {items !== null && <ClipList items={items} />}
    </PhonePage>
  );
}
