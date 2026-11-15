import { useContext, type ReactNode } from 'react';
import { createPortal } from 'react-dom';
import { useTranslation } from 'react-i18next';
import { Button } from '../Button/Button';
import { cx } from '../cx';
import { Icon } from '../Icon/Icon';
import type { IconName } from '../Icon/icons';
import { ToastDock } from '../layout/Window';
import './Toast.css';

export interface ToastProps {
  /** What just happened ("Copied brew install neovim"). */
  readonly children: ReactNode;
  readonly onDismiss: () => void;
  /** One quiet action before Dismiss ("Forget"). */
  readonly action?: ReactNode;
  /** Float above the action bar (default); `false` renders it in place. */
  readonly docked?: boolean;
  /** What it is about (default: a tick, something worked). */
  readonly icon?: IconName;
  /** The way out, when it is an answer ("Decline"); default "Dismiss". */
  readonly dismissLabel?: string;
}

/** A short confirmation that something just worked. Announced politely; rises in with a spring. */
export function Toast({
  children,
  onDismiss,
  action,
  docked = true,
  icon = 'check',
  dismissLabel,
}: ToastProps) {
  const { t } = useTranslation();
  const dock = useContext(ToastDock);
  const toast = (
    <div className={cx(docked && dock === null && 'p-toast-dock')} role="status">
      <span className="p-toast">
        <Icon name={icon} />
        <span>{children}</span>
        {action}
        <Button kind="quiet" size="sm" onClick={onDismiss}>
          {dismissLabel ?? t('ui.dismiss')}
        </Button>
      </span>
    </div>
  );
  return docked && dock !== null ? createPortal(toast, dock) : toast;
}
