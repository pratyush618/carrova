import { useTranslation } from 'react-i18next';
import { Button, Disclose, MapRow, Sheet, Stack } from '../../ui';
import type { MappedPath, MappingModel, MappingRow } from './model';

/**
 * "Where things land": each cross-system item's path there above its path
 * here, the changed part highlighted. Items that keep their place fold away.
 */
export function MappingSheet({
  model,
  onClose,
}: {
  readonly model: MappingModel;
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  const from = t(`settings.about.os.${model.from}`);
  const to = t(`settings.about.os.${model.to}`);
  // The source's own folders are unknown here, so its side names the semantic folder.
  const shown = (p: MappedPath): string =>
    p.abs ?? `${t(`receive.mapping.roots.${p.root}`)}${p.rel === '' ? '' : `/${p.rel}`}`;
  const row = (r: MappingRow, same: boolean) => (
    <MapRow
      key={r.key}
      glyph={r.glyph}
      title={r.title}
      from={{ os: model.from, label: from, path: same ? shown(r.to) : shown(r.from) }}
      to={{ os: model.to, label: to, path: shown(r.to) }}
    />
  );
  return (
    <Sheet
      wide
      title={t('receive.mapping.title', { to })}
      sub={t('receive.mapping.sub', { from })}
      onClose={onClose}
      foot={
        <Button kind="primary" onClick={onClose}>
          {t('action.done')}
        </Button>
      }
    >
      <Stack>
        {model.moved.map((r) => row(r, false))}
        {model.same.length > 0 && (
          <Disclose summary={t('receive.mapping.same', { count: model.same.length })}>
            <Stack>{model.same.map((r) => row(r, true))}</Stack>
          </Disclose>
        )}
      </Stack>
    </Sheet>
  );
}
