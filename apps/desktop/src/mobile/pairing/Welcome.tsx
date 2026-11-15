import { useTranslation } from 'react-i18next';
import { Button, Empty } from '../../ui';
import { PhonePage } from '../PhonePage';

/** The first screen while no computer is linked: one promise, one action. */
export function Welcome({ onAdd }: { readonly onAdd: () => void }) {
  const { t } = useTranslation();
  return (
    <PhonePage center>
      <Empty
        art="swap"
        title={t('mobile.welcome.title')}
        body={t('mobile.welcome.body')}
        action={
          <div className="m-wide">
            <Button kind="primary" size="lg" onClick={onAdd}>
              {t('mobile.welcome.add')}
            </Button>
            <p className="m-note">{t('mobile.welcome.hint')}</p>
          </div>
        }
      />
    </PhonePage>
  );
}
