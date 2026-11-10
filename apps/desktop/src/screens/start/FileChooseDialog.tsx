import { useTranslation } from 'react-i18next';
import { Button, Dialog, Icon, LinkRow, List } from '../../ui';
import type { FileChooseIntents } from './model';

/** For computers that can't see each other: two choices in a dialog. */
export function FileChooseDialog({ on }: { readonly on: FileChooseIntents }) {
  const { t } = useTranslation();
  const chev = <Icon name="chev" />;
  return (
    <Dialog
      icon="file"
      title={t('start.fileChoose.title')}
      onDismiss={on.onCancel}
      foot={
        <Button kind="quiet" onClick={on.onCancel}>
          {t('action.cancel')}
        </Button>
      }
    >
      <p>{t('start.fileChoose.body')}</p>
      <List>
        <LinkRow
          lead={<Icon name="send" />}
          title={t('start.fileChoose.save.title')}
          sub={t('start.fileChoose.save.sub')}
          trail={chev}
          onClick={on.onSave}
        />
        <LinkRow
          lead={<Icon name="receive" />}
          title={t('start.fileChoose.open.title')}
          sub={t('start.fileChoose.open.sub')}
          trail={chev}
          onClick={on.onOpen}
        />
      </List>
    </Dialog>
  );
}
