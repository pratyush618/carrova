import { useTranslation } from 'react-i18next';
import { Badge, type IconName, type Tone } from '../../ui';
import type { MoveTag } from './moveWords';

/** How each exception reads: interrupted asks for attention, the others are quiet facts. */
const LOOK: Record<MoveTag, { readonly tone: Tone; readonly icon?: IconName }> = {
  interrupted: { tone: 'warning' },
  undone: { tone: 'neutral', icon: 'undo' },
  undo_expired: { tone: 'neutral' },
};

/** The chip that calls out a move's exception. */
export function MoveTagBadge({ tag }: { readonly tag: MoveTag }) {
  const { t } = useTranslation();
  const { tone, icon } = LOOK[tag];
  return (
    <Badge tone={tone} {...(icon && { icon })}>
      {t(`history.tag.${tag}`)}
    </Badge>
  );
}
