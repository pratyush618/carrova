import type { NotifyAppRow } from '../../bindings/NotifyAppRow';
import { useTranslation } from 'react-i18next';
import { AppGlyph, Button, List, Muted, Row, Sheet } from '../../ui';

/** A linked phone's per-app notification list, read only: it is changed on the phone. */
export function PhoneAppsSheet({
  name,
  apps,
  onClose,
}: {
  /** The phone's name. */
  readonly name: string;
  /** By name, as the engine sorts them. */
  readonly apps: readonly NotifyAppRow[];
  readonly onClose: () => void;
}) {
  const { t } = useTranslation();
  return (
    <Sheet
      title={t('ecosystem.phoneApps.title', { name })}
      sub={t('ecosystem.phoneApps.sub', { name })}
      onClose={onClose}
      foot={
        <Button kind="primary" onClick={onClose}>
          {t('ecosystem.phoneApps.done')}
        </Button>
      }
    >
      {apps.length === 0 ? (
        <Muted>{t('ecosystem.phoneApps.none')}</Muted>
      ) : (
        <List>
          {apps.map((app) => (
            <Row
              key={app.id}
              lead={<AppGlyph glyph={app.name.slice(0, 2)} size="sm" />}
              title={app.name}
              sub={t(
                app.on
                  ? 'ecosystem.phoneApps.on'
                  : app.private
                    ? 'ecosystem.phoneApps.private'
                    : 'ecosystem.phoneApps.off',
              )}
            />
          ))}
        </List>
      )}
    </Sheet>
  );
}
