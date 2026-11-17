import { useTranslation } from 'react-i18next';
import { Button } from '../../../ui';
import { initials } from '../messages/ThreadList';
import './calls.css';

/** A call this computer rang for and nobody answered: call back from the phone, or close. */
export function MissedCall({
  who,
  name,
  phone,
  on,
}: {
  readonly who: string;
  /** The caller's contact name (`null`: unknown), for the initials. */
  readonly name: string | null;
  readonly phone: string;
  readonly on: { readonly onCallBack: () => void; readonly onClose: () => void };
}) {
  const { t } = useTranslation();
  return (
    <main className="s-call">
      <span className="s-call__av" aria-hidden="true">
        {initials(name)}
      </span>
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
