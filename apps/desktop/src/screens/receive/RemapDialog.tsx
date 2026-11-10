import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import type { Remap, SemanticRoot } from '../../engine/types';
import { Button, Dialog, Field, Input, Select, StandaloneRow, type SelectOption } from '../../ui';

const ROOTS: readonly SemanticRoot[] = ['user_config', 'user_data', 'home', 'documents', 'cache'];

export interface RemapModel {
  /** The item's name. */
  readonly item: string;
  /** The remap in effect, if any. */
  readonly current: Remap | null;
}

export interface RemapIntents {
  readonly onCancel: () => void;
  readonly onSave: (remap: Remap) => void;
  /** Put the item back where it came from. */
  readonly onReset: () => void;
}

/** Choose where an item goes instead: a known folder and a sub-folder inside it. */
export function RemapDialog({
  model,
  on,
}: {
  readonly model: RemapModel;
  readonly on: RemapIntents;
}) {
  const { t } = useTranslation();
  const [root, setRoot] = useState<SemanticRoot>(model.current?.root ?? 'documents');
  const [prefix, setPrefix] = useState(model.current?.rel_prefix ?? '');
  const roots: SelectOption<SemanticRoot>[] = ROOTS.map((value) => ({
    value,
    label: t(`receive.remap.roots.${value}`),
  }));
  const rel = prefix.trim();
  return (
    <Dialog
      icon="folder"
      title={t('receive.remap.title', { item: model.item })}
      onDismiss={on.onCancel}
      foot={
        <>
          {model.current !== null && (
            <Button kind="quiet" onClick={on.onReset}>
              {t('receive.remap.reset')}
            </Button>
          )}
          <Button data-initial-focus onClick={on.onCancel}>
            {t('action.cancel')}
          </Button>
          <Button
            kind="primary"
            onClick={() => {
              on.onSave({ root, rel_prefix: rel === '' ? null : rel });
            }}
          >
            {t('receive.remap.save')}
          </Button>
        </>
      }
    >
      <p>{t('receive.remap.body')}</p>
      <StandaloneRow
        title={t('receive.remap.root')}
        trail={
          <Select label={t('receive.remap.root')} options={roots} value={root} onChange={setRoot} />
        }
      />
      <Field label={t('receive.remap.prefix')} hint={t('receive.remap.prefixHint')}>
        {(control) => (
          <Input
            {...control}
            mono
            value={prefix}
            onChange={(event) => {
              setPrefix(event.target.value);
            }}
          />
        )}
      </Field>
    </Dialog>
  );
}
