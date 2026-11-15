import type { ClipItemView } from '../../engine/types';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { AppGlyph, Button, List, Muted, Row, Sheet } from '../../ui';

/** Clips received from linked devices, newest first, kept on this computer only. */
export function ClipHistorySheet({
  items,
  on,
}: {
  /** Newest first, as the engine keeps them. */
  readonly items: readonly ClipItemView[];
  readonly on: { readonly onClear: () => void; readonly onClose: () => void };
}) {
  const { t } = useTranslation();
  return (
    <Sheet
      title={t('ecosystem.history.title')}
      sub={t('ecosystem.history.sub')}
      onClose={on.onClose}
      foot={
        <>
          {items.length > 0 && (
            <Button kind="quiet" onClick={on.onClear}>
              {t('ecosystem.history.clear')}
            </Button>
          )}
          <Button kind="primary" onClick={on.onClose}>
            {t('ecosystem.history.done')}
          </Button>
        </>
      }
    >
      <ClipList items={items} />
    </Sheet>
  );
}

/** The received clips, newest first (or a line saying there are none). */
export function ClipList({ items }: { readonly items: readonly ClipItemView[] }) {
  const { t } = useTranslation();
  const format = useFormat();
  return items.length === 0 ? (
    <Muted>{t('ecosystem.history.empty')}</Muted>
  ) : (
    <List>
      {items.map((item, i) => (
        <Row
          // Clips have no id; the list only changes as a whole.
          key={`${String(i)}-${String(item.at)}`}
          lead={<AppGlyph glyph={{ icon: item.kind === 'image' ? 'eye' : 'copy' }} size="sm" />}
          title={item.kind === 'image' ? t('ecosystem.history.image') : item.preview}
          sub={format.time(new Date(item.at * 1000))}
        />
      ))}
    </List>
  );
}
