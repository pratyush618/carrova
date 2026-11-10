import type { ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { Button } from '../Button/Button';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import './Toast.css';

export interface ToastProps {
  /** What just happened ("Copied brew install neovim"). */
  readonly children: ReactNode;
  readonly onDismiss: () => void;
  /** Float above the action bar (default); `false` renders it in place. */
  readonly docked?: boolean;
}

/** A short confirmation that something just worked. Announced politely; rises in with a spring. */
export function Toast({ children, onDismiss, docked = true }: ToastProps) {
  const { t } = useTranslation();
  return (
    <div className={cx(docked && 'p-toast-dock')} role="status">
      <span className="p-toast">
        <Icon name="check" />
        <span>{children}</span>
        <Button kind="quiet" size="sm" onClick={onDismiss}>
          {t('ui.dismiss')}
        </Button>
      </span>
    </div>
  );
}
