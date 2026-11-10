import { useTranslation } from 'react-i18next';
import { Badge } from '../../ui';
import type { Tier } from './model';

/** A recipe's trust tier, the same badge in the grid and the detail sheet. */
export function TierBadge({ tier }: { readonly tier: Tier }) {
  const { t } = useTranslation();
  switch (tier) {
    case 'verified':
      return (
        <Badge tone="success" icon="check">
          {t('badge.verified')}
        </Badge>
      );
    case 'community':
      return <Badge>{t('badge.community')}</Badge>;
    case 'unverified':
      return <Badge tone="warning">{t('badge.unverified')}</Badge>;
  }
}
