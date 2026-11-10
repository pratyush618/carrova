import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Button, Icon, LinkRow, List, Row, Search, Sheet, SheetNote, Toggle } from '../../ui';
import { CustomizeTree } from './CustomizeTree';
import type { CustomizeIntents, CustomizeModel } from './model';

export interface CustomizeSheetProps {
  readonly model: CustomizeModel;
  readonly on: CustomizeIntents;
}

/** The full selection: single files, sizes, flags, raw folders and caches. */
export function CustomizeSheet({ model, on }: CustomizeSheetProps) {
  const { t } = useTranslation();
  const format = useFormat();
  return (
    <Sheet
      title={t('send.customize.title')}
      sub={t('send.customize.sub')}
      onClose={on.onDone}
      foot={
        <>
          <Button kind="quiet" onClick={on.onReset}>
            {t('send.customize.reset')}
          </Button>
          <SheetNote>
            {t('send.customize.picked', {
              picked: format.number(model.picked.files),
              total: format.number(model.picked.totalFiles),
              size: format.bytes(model.picked.bytes),
            })}
          </SheetNote>
          <Button kind="primary" onClick={on.onDone}>
            {t('action.done')}
          </Button>
        </>
      }
    >
      <Search
        label={t('send.customize.filter')}
        placeholder={t('send.customize.filterPlaceholder')}
        value={model.filter}
        onChange={on.onFilter}
      />
      <CustomizeTree apps={model.apps} on={on} />
      <List>
        <LinkRow
          lead={<Icon name="folder" />}
          title={t('send.customize.unrecognised')}
          sub={t('send.customize.unrecognisedSub', {
            count: model.unrecognised.count,
            size: format.bytes(model.unrecognised.minBytes),
          })}
          trail={<Icon name="chev" />}
          onClick={on.onUnrecognised}
        />
        <Row
          lead={<Icon name="disk" />}
          title={t('send.customize.caches')}
          sub={t('send.customize.cachesSub', { size: format.bytes(model.cacheBytes) })}
          trail={
            <Toggle
              on={model.includeCaches}
              label={t('send.customize.caches')}
              onChange={on.onIncludeCaches}
            />
          }
        />
      </List>
    </Sheet>
  );
}
