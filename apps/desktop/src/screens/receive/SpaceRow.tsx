import './receive.css';
import { useTranslation } from 'react-i18next';
import { useFormat } from '../../i18n/useFormat';
import { Icon, LinkButton } from '../../ui';
import { cx } from '../../ui/cx';
import type { ReviewModel } from './model';

/**
 * Space needed against free, as a line and a meter. When short, the override
 * (behind its warning) sits right here.
 */
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
  const state = space.ignored
    ? 'ignored'
    : space.free === null
      ? 'unknown'
      : space.short
        ? 'short'
        : 'plenty';
  const share =
    space.free === null || space.free === 0 ? 1 : Math.min(1, space.needed / space.free);
  return (
    <div className={cx('s-space', `s-space--${state}`)}>
      <p className="s-space__top">
        <Icon name={state === 'plenty' ? 'check' : 'disk'} size="sm" />
        <b>{t(`receive.review.space.${state}`)}</b>
        <span>
          {space.free === null
            ? t('receive.review.space.subUnknown', { needed })
            : t('receive.review.space.sub', { needed, free: format.bytes(space.free) })}
        </span>
        {space.short && !space.ignored && (
          <LinkButton onClick={onIgnore}>{t('transfer.ignoreSpace')}</LinkButton>
        )}
      </p>
      {space.free !== null && (
        <span className="s-space__bar" aria-hidden="true">
          <i style={{ '--p': String(share) }} />
        </span>
      )}
    </div>
  );
}
