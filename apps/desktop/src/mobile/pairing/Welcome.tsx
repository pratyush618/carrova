import { useTranslation } from 'react-i18next';
import { Button, Empty, LinkButton } from '../../ui';
import { PhonePage } from '../PhonePage';

/** The first screen while no computer is linked: one promise, one action, and app updates. */
export function Welcome({
  update,
  onAdd,
  onUpdate,
}: {
  /** The version on offer, if any. */
  readonly update: string | null;
  readonly onAdd: () => void;
  /** Open the update page. */
  readonly onUpdate: () => void;
}) {
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
            <LinkButton onClick={onUpdate}>
              {update === null
                ? t('mobile.welcome.update')
                : t('mobile.welcome.updateReady', { version: update })}
            </LinkButton>
          </div>
        }
      />
    </PhonePage>
  );
}
