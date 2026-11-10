import { useTranslation } from 'react-i18next';
import { Icon, Kbd } from '../../ui';

/** Two computers with an arrow leaving (send) or arriving (receive). Decorative. */
function Art({ verb }: { readonly verb: 'send' | 'receive' }) {
  const x = verb === 'send' ? 2 : 30;
  return (
    <svg className="s-verb__art" width="64" height="44" viewBox="0 0 64 44" aria-hidden="true">
      <rect className="s-verb__screen" x={x} y="6" width="30" height="20" rx="4" />
      <rect className="s-verb__base" x={x} y="29" width="30" height="3" rx="1.5" />
      <path
        className="s-verb__arrow"
        d={verb === 'send' ? 'M36 16h20m-6-6 6 6-6 6' : 'M6 16h20m-6-6 6 6-6 6'}
      />
    </svg>
  );
}

export interface VerbTileProps {
  readonly verb: 'send' | 'receive';
  /** The single-key shortcut shown on the tile (handled by Home). */
  readonly shortcut: string;
  /** First run: a pointer to the likely first step, dismissible. */
  readonly hint?: { readonly label: string; readonly onDismiss: () => void };
  readonly onClick: () => void;
}

/** One of Home's two verbs: a big tile with its art and keyboard shortcut. */
export function VerbTile({ verb, shortcut, hint, onClick }: VerbTileProps) {
  const { t } = useTranslation();
  return (
    <div className="s-verb">
      {hint && (
        <p className="s-verb__hint">
          {hint.label}
          <button type="button" aria-label={t('ui.dismiss')} onClick={hint.onDismiss}>
            <Icon name="close" size="sm" />
          </button>
        </p>
      )}
      <button
        type="button"
        className={verb === 'send' ? 's-verb__tile s-verb__tile--primary' : 's-verb__tile'}
        aria-keyshortcuts={shortcut}
        onClick={onClick}
      >
        <Art verb={verb} />
        <span className="s-verb__kbd" aria-hidden="true">
          <Kbd>{shortcut}</Kbd>
        </span>
        <span className="s-verb__title">{t(`start.home.${verb}.title`)}</span>
        <span className="s-verb__sub">{t(`start.home.${verb}.sub`)}</span>
      </button>
    </div>
  );
}
