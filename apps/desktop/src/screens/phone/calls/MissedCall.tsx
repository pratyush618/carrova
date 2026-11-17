import { useTranslation } from 'react-i18next';
import { Button } from '../../../ui';
import './calls.css';

/** A call this computer rang for and nobody answered: call back from the phone, or close. */
export function MissedCall({
  who,
  phone,
  on,
}: {
  readonly who: string;
  readonly phone: string;
  readonly on: { readonly onCallBack: () => void; readonly onClose: () => void };
}) {
  const { t } = useTranslation();
  return (
    <main className="s-call">
      <h1 className="s-call__who">{t('calls.window.missedFrom', { who })}</h1>
      <p className="s-call__sub">{phone}</p>
      <div className="s-call__acts">
        <Button onClick={on.onClose}>{t('calls.window.close')}</Button>
        <Button kind="primary" icon="call" onClick={on.onCallBack}>
          {t('calls.window.callBack')}
        </Button>
      </div>
    </main>
  );
}
