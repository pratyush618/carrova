import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Badge, Icon, LinkButton, Row } from '../../ui';
import type { ReviewModel } from './model';

/** Space needed vs free. When short, the override (behind its warning) sits right here. */
export function SpaceRow({
  space,
  onIgnore,
}: {
  readonly space: ReviewModel['space'];
  readonly onIgnore: () => void;
}) {
  const { t } = useTranslation();
  const format = useFormat();
  const needed = format.bytes(space.needed);
  const badge = space.ignored ? (
    <Badge tone="warning">{t('badge.ignored')}</Badge>
  ) : space.short ? (
    <Badge tone="danger">{t('badge.notEnough')}</Badge>
  ) : (
    <Badge tone="success">{t('badge.enough')}</Badge>
  );
  return (
    <Row
      lead={<Icon name="disk" />}
      title={t('receive.review.space')}
      sub={
        <>
          {space.free === null
            ? t('receive.review.spaceUnknown', { needed })
            : t('receive.review.spaceSub', { needed, free: format.bytes(space.free) })}{' '}
          {space.short && !space.ignored && (
            <LinkButton onClick={onIgnore}>{t('transfer.ignoreSpace')}</LinkButton>
          )}
        </>
      }
      trail={badge}
    />
  );
}
