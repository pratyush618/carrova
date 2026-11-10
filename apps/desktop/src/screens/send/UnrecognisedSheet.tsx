import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import {
  Button,
  Check,
  List,
  Muted,
  Path,
  Row,
  Select,
  Sheet,
  StandaloneRow,
  type SelectOption,
} from '../../ui';
import type { MinSize, UnrecognisedIntents, UnrecognisedModel } from './model';

const SIZES: readonly MinSize[] = ['10mb', '100mb', 'any'];

/** Raw folders, for data no recipe covers. A size filter keeps the list short. */
export function UnrecognisedSheet({
  model,
  on,
}: {
  readonly model: UnrecognisedModel;
  readonly on: UnrecognisedIntents;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const sizes: SelectOption<MinSize>[] = SIZES.map((value) => ({
    value,
    label: t(`send.unrecognised.sizes.${value}`),
  }));
  return (
    <Sheet
      title={t('send.unrecognised.title')}
      onClose={on.onDone}
      foot={
        <Button kind="primary" onClick={on.onDone}>
          {t('action.done')}
        </Button>
      }
    >
      <Muted>{t('send.unrecognised.intro')}</Muted>
      <StandaloneRow
        title={t('send.unrecognised.minSize')}
        trail={
          <Select
            label={t('send.unrecognised.minSize')}
            options={sizes}
            value={model.minSize}
            onChange={on.onMinSize}
          />
        }
      />
      <List label={t('send.unrecognised.folders')}>
        {model.folders.map((folder) => (
          <Row
            key={folder.key}
            lead={
              <Check
                value={folder.picked}
                label={folder.path}
                onChange={(picked) => {
                  on.onToggle(folder.key, picked);
                }}
              />
            }
            title={<Path>{folder.path}</Path>}
            sub={folder.hint}
            trail={format.bytes(folder.bytes)}
          />
        ))}
      </List>
    </Sheet>
  );
}
